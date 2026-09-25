//! Build-time ROM validation and asset extraction.
//!
//! The user supplies their own legally owned `.z64`. This tool reads it and
//! writes derived assets into `assets/generated/`, which is gitignored. The
//! ROM is never copied into the repository, and neither is anything extracted
//! from it.
//!
//! ```text
//! romtool verify   <rom>          identify the ROM and print its header
//! romtool info     <rom>          summarise the relocData archive
//! romtool extract  <rom> [--out]  extract every archive file + a manifest
//! romtool dump     <rom> <id>     dump one archive file
//! romtool textures <rom>          extract + pack every bound texture
//! romtool scene    <rom>          recover DObjDesc scene graphs
//! romtool texgen   <rom>          census G_TEXTURE_GEN vertex-load state
//! romtool stages   <rom>          recover MPGroundData headers and collision
//! romtool collide  <pack>         run the collision query on every stage
//! romtool simulate <pack>         drop a real fighter on every stage's spawns
//! romtool jumptest <pack>         script a jump/attack input schedule against
//!                                 a stage's real floor data and report where
//!                                 it lands and whether a jab connects
//! romtool fighters <rom>          extract every character's FTAttributes
//! romtool anims    <rom>          read every fighter's animation lengths
//! romtool effects  <pack>         verify source-named manager effect objects
//! romtool particles <rom>         validate LBParticle script/texture banks
//! ```

// Research tables and report aggregations use ad-hoc tuples on purpose.
#![allow(clippy::type_complexity)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ssb_rom::archive::Archive;
use ssb_rom::rom;

mod filter_coverage;
mod matcolors;
mod matsample;
mod residual_fixes;
mod residuals;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let result = match refs.as_slice() {
        ["verify", rom_path] => verify(rom_path.as_ref()),
        ["info", rom_path] => info(rom_path.as_ref()),
        ["check", rom_path] => check(rom_path.as_ref()),
        ["scan", rom_path, rest @ ..] => scan(rom_path.as_ref(), rest),
        ["mesh", rom_path] => mesh(rom_path.as_ref()),
        ["scene", rom_path, rest @ ..] => scene(rom_path.as_ref(), rest),
        ["mobj", rom_path, rest @ ..] => mobj(rom_path.as_ref(), rest),
        ["texgen", rom_path, rest @ ..] => texgen(rom_path.as_ref(), rest),
        ["stages", rom_path, rest @ ..] => stages(rom_path.as_ref(), rest),
        ["matcolors", rom_path, rest @ ..] => matcolors::matcolors(rom_path.as_ref(), rest),
        ["pack", rom_path, rest @ ..] => pack(rom_path.as_ref(), rest),
        ["residuals", rom_path, rest @ ..] => residuals_cmd(rom_path.as_ref(), rest),
        ["collide", pack_path, rest @ ..] => collide(pack_path.as_ref(), rest),
        ["jumptest", pack_path, rest @ ..] => jumptest(pack_path.as_ref(), rest),
        ["simulate", pack_path, rest @ ..] => simulate(pack_path.as_ref(), rest),
        ["effects", pack_path] => effects(pack_path.as_ref()),
        ["particles", rom_path] => particles(rom_path.as_ref()),
        ["fighters", rom_path, rest @ ..] => fighters(rom_path.as_ref(), rest),
        ["anims", rom_path, rest @ ..] => anims(rom_path.as_ref(), rest),
        ["anim-length", rom_path, rest @ ..] => anim_length(rom_path.as_ref(), rest),
        ["figatree", rom_path, rest @ ..] => figatree(rom_path.as_ref(), rest),
        ["texdump", rom_path, rest @ ..] => texdump(rom_path.as_ref(), rest),
        ["extract", rom_path, rest @ ..] => extract(rom_path.as_ref(), rest),
        ["dump", rom_path, id] => dump(rom_path.as_ref(), id),
        ["textures", rom_path, rest @ ..] => textures(rom_path.as_ref(), rest),
        _ => {
            usage();
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!(
        "romtool -- SSB64 ROM validation and asset extraction

USAGE:
    romtool verify   <rom.z64>
    romtool info     <rom.z64>
    romtool check    <rom.z64>
    romtool scan     <rom.z64> [--exhaustive]
    romtool mesh     <rom.z64>
    romtool scene    <rom.z64> [--file <id>] [--list] [--nodes] [--why]
                               [--expect <ground-truth.tsv>]
    romtool mobj     <rom.z64> [--file <id>] [--expect <ground-truth.tsv>]
                               [--search] [--expect-tables <tables.tsv>]
    romtool texgen   <rom.z64> [--file <id>] [--lines] [--pack <pack.pak>] [--verify]
    romtool stages   <rom.z64> [--file <id>] [--lines] [--pack <pack.pak>]
    romtool matcolors <rom.z64> --pack <pack.pak>
    romtool pack     <rom.z64> [--out <file>] [--file <id>] [--no-swizzle]
    romtool collide  <pack.pak> [--stage <n>]
    romtool simulate <pack.pak> [--stage <n>] [--verbose]
    romtool jumptest <pack.pak> [--stage <n>] [--jump-tick <n>] [--jump2-tick <n>]
                                 [--stick-x <n>] [--stick-switch-tick <n>]
                                 [--stick-release-tick <n>] [--attack-tick <n>] [--ticks <n>]
    romtool effects  <pack.pak>
    romtool particles <rom.z64>
    romtool fighters <rom.z64> [--verify] [--refs <relocData dir>]
    romtool anims    <rom.z64> [--verify]
    romtool anim-length <rom.z64> <file-id> [file-id ...]
    romtool figatree <rom.z64> [--fighter <name>] [--slot <name>] [--frames <n>]
                               [--pack <pack.pak>]
    romtool extract  <rom.z64> [--out <dir>] [--limit <n>]
    romtool dump     <rom.z64> <file-id>
    romtool textures <rom.z64> [--file <id>]
    romtool texdump  <rom.z64> [--file <id>] [--count <n>]

The ROM is read only. Output defaults to assets/generated/, which is
gitignored -- no extracted asset is ever committed."
    );
}

type Res = Result<(), Box<dyn std::error::Error>>;

fn load_rom(path: &Path) -> Result<(Vec<u8>, rom::RomInfo), Box<dyn std::error::Error>> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let info = rom::identify(&data)?;
    Ok((data, info))
}

fn verify(path: &Path) -> Res {
    let (_, info) = load_rom(path)?;
    println!("ROM:      {}", path.display());
    println!("Region:   {:?}", info.region);
    println!("Name:     {}", info.internal_name);
    println!(
        "Code:     {}",
        String::from_utf8_lossy(info.region.game_code())
    );
    println!("SHA-1:    {}", info.sha1);
    println!("Status:   supported");
    Ok(())
}

fn info(path: &Path) -> Res {
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let entries = archive.entries();
    let compressed = entries.iter().filter(|e| e.compressed).count();
    let total_rom: usize = entries.iter().map(|e| e.rom_size()).sum();
    let total_ram: usize = entries.iter().map(|e| e.size()).sum();
    let largest = entries
        .iter()
        .enumerate()
        .max_by_key(|(_, e)| e.size())
        .expect("archive is non-empty");

    println!("relocData archive");
    println!("  table offset   0x{:08X}", archive.table_offset());
    println!("  data base      0x{:08X}", archive.data_base());
    println!("  files          {}", archive.len());
    println!(
        "  compressed     {compressed} ({:.1}%)",
        compressed as f64 / archive.len() as f64 * 100.0
    );
    println!(
        "  packed size    {:.2} MiB",
        total_rom as f64 / (1 << 20) as f64
    );
    println!(
        "  unpacked size  {:.2} MiB",
        total_ram as f64 / (1 << 20) as f64
    );
    println!(
        "  ratio          {:.2}x",
        total_ram as f64 / total_rom as f64
    );
    println!(
        "  largest file   #{} at {} KiB",
        largest.0,
        largest.1.size() / 1024
    );
    Ok(())
}

/// Validates every archive file, cross-checking decompression against ROM
/// geometry. See `Archive::verify_extern_chain` for why this is a meaningful
/// test of the VPK0 decoder and not just a smoke test.
fn check(path: &Path) -> Res {
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let mut load_failures = Vec::new();
    let mut chain_mismatches = Vec::new();
    let mut compressed_verified = 0usize;
    let mut total_extern = 0usize;
    let mut total_intern = 0usize;

    for id in 0..archive.len() as u32 {
        let compressed = archive.entry(id).is_some_and(|e| e.compressed);

        match archive.verify_extern_chain(id) {
            Ok((chain, id_list)) => {
                if chain != id_list {
                    chain_mismatches.push((id, chain, id_list));
                } else if compressed {
                    compressed_verified += 1;
                }
                total_extern += chain;
            }
            Err(e) => load_failures.push((id, e)),
        }

        if let Ok(f) = archive.load(id) {
            total_intern += f.intern_relocs.len();
        }
    }

    println!("archive self-check");
    println!("  files                 {}", archive.len());
    println!("  load failures         {}", load_failures.len());
    println!("  intern reloc slots    {total_intern}");
    println!("  extern reloc slots    {total_extern}");
    println!("  chain/ROM mismatches  {}", chain_mismatches.len());
    println!("  compressed files cross-verified against ROM geometry: {compressed_verified}");

    for (id, e) in load_failures.iter().take(10) {
        eprintln!("  load failure: file {id}: {e}");
    }
    for (id, chain, list) in chain_mismatches.iter().take(10) {
        eprintln!("  mismatch: file {id}: chain has {chain}, ROM implies {list}");
    }

    if load_failures.is_empty() && chain_mismatches.is_empty() {
        println!("\nOK: every file decompressed and relocated consistently.");
        Ok(())
    } else {
        Err("archive self-check failed".into())
    }
}

/// Inventories every display list in the archive.
///
/// This is the measurement plan §8 asks for: rather than speculatively
/// supporting the whole RDP feature set, find out what Smash actually emits and
/// build a converter for that.
fn scan(path: &Path, opts: &[&str]) -> Res {
    let how = if opts.contains(&"--exhaustive") {
        ssb_rom::scan::Candidates::Exhaustive
    } else {
        ssb_rom::scan::Candidates::RelocTargets
    };
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    // Load everything once, then build the cross-file pointer graph. A display
    // list can live in one file and be referenced only from another, so
    // scanning files in isolation misses them.
    let files: Vec<_> = (0..archive.len() as u32)
        .filter_map(|id| archive.load(id).ok())
        .collect();
    let mut inv = ssb_rom::scan::Inventory::default();
    let mut biggest: Vec<(u32, usize)> = Vec::new();

    for file in &files {
        let dls = if how == ssb_rom::scan::Candidates::Exhaustive {
            ssb_rom::scan::find_display_lists_with(file, how)
        } else {
            ssb_rom::scan::find_root_display_lists(file)
        };

        if !dls.is_empty() {
            let tris: usize = dls.iter().map(|d| d.triangle_count()).sum();
            biggest.push((file.id, tris));
        }
        inv.add_file(&dls);
    }

    println!("display list inventory ({} archive files)", archive.len());
    println!("  files containing DLs   {}", inv.files_with_dls);
    println!("  display lists          {}", inv.display_lists);
    println!("  triangles              {}", inv.triangles);
    println!(
        "  vertex loads           {} ({} vertices)",
        inv.vertex_loads, inv.vertices_loaded
    );
    println!("  max vertices per G_VTX {}", inv.max_vtx_batch);
    println!("  longest DL             {} commands", inv.max_commands);

    println!("\nopcodes actually used:");
    let mut ops: Vec<_> = inv.opcodes.iter().collect();
    ops.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (op, n) in ops {
        println!(
            "  0x{op:02X} {:<18} {n:>8}",
            ssb_rom::scan::opcode_name(*op)
        );
    }

    println!("\ntexture formats (G_SETTILE):");
    if inv.texture_formats.is_empty() {
        println!("  (none)");
    }
    let mut fmts: Vec<_> = inv.texture_formats.iter().collect();
    fmts.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for ((f, s), n) in fmts {
        let name = ssb_rom::texture::Format::from_raw(*f)
            .map(|f| format!("{f:?}"))
            .unwrap_or_else(|| format!("raw{f}"));
        let bits = ssb_rom::texture::BitSize::from_raw(*s)
            .map(|b| b.bits())
            .unwrap_or(0);
        println!("  {name:<5}{bits:>3}bpp  {n:>8}");
    }

    println!("\nTLUT load sizes (palette entries):");
    for (size, n) in &inv.tlut_sizes {
        println!("  {size:>4} entries  {n:>8}");
    }

    println!("\ngeometry mode bits set:");
    for (bit, n) in &inv.geometry_mode_set {
        println!("  0x{bit:08X}  {n:>8}  {}", geometry_mode_name(*bit));
    }

    biggest.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    println!("\nheaviest files by triangle count:");
    for (id, tris) in biggest.iter().take(12) {
        println!("  file {id:<5} {tris:>7} triangles");
    }

    Ok(())
}

/// F3DEX2 geometry mode bit names (`gbi.h`).
fn geometry_mode_name(bit: u32) -> &'static str {
    match bit {
        0x0000_0001 => "G_ZBUFFER",
        // R0.16/RE-119: was `0x0000_0002`, disagreeing with
        // `refs/ssb-decomp-re/include/PR/gbi.h`'s own `#define G_SHADE
        // 0x00000004` -- the display-only mislabel this fixes hid 60
        // archive-wide occurrences under a blank name instead of `G_SHADE`.
        0x0000_0004 => "G_SHADE",
        0x0000_0200 => "G_CULL_FRONT",
        0x0000_0400 => "G_CULL_BACK",
        0x0001_0000 => "G_FOG",
        0x0002_0000 => "G_LIGHTING",
        0x0004_0000 => "G_TEXTURE_GEN",
        0x0008_0000 => "G_TEXTURE_GEN_LINEAR",
        0x0020_0000 => "G_SHADING_SMOOTH",
        0x0100_0000 => "G_CLIPPING",
        _ => "",
    }
}

/// Converts every discovered display list into indexed meshes and reports how
/// well the conversion compresses the geometry.
///
/// The headline number is the vertex dedup ratio: the RSP re-uploads shared
/// vertices constantly because its cache holds only 32, and undoing that is
/// pure win on PSP -- less memory and less GE vertex fetch.
/// Recovers `DObjDesc` scene graphs, optionally checking them against the
/// arrays the decomp has typed by hand.
///
/// The `--expect` file is TSV of `file<TAB>offset<TAB>entries<TAB>name`,
/// generated from `refs/ssb-decomp-re/src/relocData/*.c`. It is ground truth in
/// the strongest available sense: those declarations are byte-compared against
/// the original ROM on every decomp build, so an offset in that list is a place
/// a `DObjDesc` array provably starts.
fn scene(path: &Path, args: &[&str]) -> Res {
    use ssb_rom::scene;

    let mut only_file: Option<u32> = None;
    let mut expect: Option<PathBuf> = None;
    let mut list = false;
    let mut nodes = false;
    let mut why = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--file" => only_file = it.next().map(|v| v.parse()).transpose()?,
            "--expect" => expect = it.next().map(PathBuf::from),
            "--list" => list = true,
            "--nodes" => nodes = true,
            "--why" => why = true,
            other => return Err(format!("unknown flag {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let ids: Vec<u32> = match only_file {
        Some(id) => vec![id],
        None => (0..archive.len() as u32).collect(),
    };

    // file -> offset -> node count (terminator excluded)
    let mut found: BTreeMap<u32, BTreeMap<u32, usize>> = BTreeMap::new();
    let mut total_nodes = 0usize;
    let mut with_dl = 0usize;
    let mut depth_hist: BTreeMap<u32, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();

    for id in &ids {
        let Ok(file) = archive.load(*id) else {
            continue;
        };
        let graphs = scene::find_scene_graphs(&file);
        if graphs.is_empty() {
            continue;
        }
        let per_file = found.entry(*id).or_default();
        for g in &graphs {
            per_file.insert(g.offset, g.nodes.len());
            total_nodes += g.nodes.len();
            with_dl += g.nodes.iter().filter(|n| n.desc.dl.is_some()).count();
            for n in &g.nodes {
                *depth_hist.entry(n.desc.depth()).or_default() += 1;
                *kinds
                    .entry(format!("{:?}", n.desc.transform_kind()))
                    .or_default() += 1;
            }
        }
    }

    let graph_count: usize = found.values().map(BTreeMap::len).sum();
    println!("Scene graphs: {graph_count} across {} files", found.len());
    println!("Nodes:        {total_nodes} ({with_dl} carrying a display list)");
    print!("Depths:      ");
    for (d, n) in &depth_hist {
        print!(" {d}:{n}");
    }
    println!();

    if only_file.is_some() || list {
        for (id, graphs) in &found {
            for (off, n) in graphs {
                println!("  file {id} @ 0x{off:X}: {n} nodes");
            }
        }
    }

    // Per-node world positions, for checking composed transforms against the
    // translate values in the decomp's DObjDesc arrays by hand.
    if nodes {
        for id in &ids {
            let Ok(file) = archive.load(*id) else {
                continue;
            };
            for g in scene::find_scene_graphs(&file) {
                println!("\nfile {id} @ 0x{:X} ({} nodes)", g.offset, g.nodes.len());
                for (i, (node, w)) in g.nodes.iter().zip(g.world_transforms()).enumerate() {
                    let t = w.translation();
                    println!(
                        "  {i:3}  depth {}  parent {:>4}  world ({:>10.1} {:>9.1} {:>9.1})  dl {}",
                        node.desc.depth(),
                        node.parent.map_or("-".into(), |p| p.to_string()),
                        t[0],
                        t[1],
                        t[2],
                        node.desc.dl.map_or("-".into(), |d| format!("0x{d:X}")),
                    );
                }
            }
        }
    }

    // Report what each node's `dl` actually resolves to. A DObj's display-list
    // field is a union -- Gfx*, Gfx**, DObjDLLink*, an animation joint -- and
    // nothing in the data discriminates it, so "does it convert" is what
    // decides how much geometry a graph can actually place.
    let mut outcome: BTreeMap<String, usize> = BTreeMap::new();
    let mut members: BTreeMap<&'static str, usize> = BTreeMap::new();
    // Materials change what converts, so resolve them the way the packer does.
    let loaded = load_all(&archive);
    let skeleton_graphs = fighter_skeleton_graphs(&loaded);
    let ground_graphs = ground_layer1_graphs(&loaded);
    let transition_graphs = lb_transition_graphs();
    for id in &ids {
        let Some(file) = loaded.files.get(*id as usize).and_then(Option::as_ref) else {
            continue;
        };
        let resolver = scene::DlResolver::new(file);
        for g in loaded.graphs.get(id).into_iter().flatten() {
            for node_dl in g.display_lists() {
                let member = resolver.resolve(node_dl);
                *members
                    .entry(match member {
                        scene::NodeDl::Links(_) => "DObjDLLink[]",
                        scene::NodeDl::Pair { .. } => "Gfx *dls[2] pre/post pair",
                        scene::NodeDl::Direct(_) => "Gfx * (direct)",
                    })
                    .or_default() += 1;
            }

            // Convert the graph the way the packer does -- in draw order,
            // sharing one vertex cache -- so this reports what actually
            // happens rather than what a standalone conversion would.
            let plan = plan_draw_order(g, &resolver);
            let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                .iter()
                .map(|p| {
                    file.data
                        .get(p.dl as usize..)
                        .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                        .unwrap_or_default()
                })
                .collect();
            let materials = loaded.materials(file, g);
            let initial = initial_material_for(
                &skeleton_graphs,
                &ground_graphs,
                &transition_graphs,
                *id,
                g.offset,
            );
            let items: Vec<ssb_rom::mesh::SequenceItem> = plan
                .iter()
                .zip(&decoded)
                .map(|(p, cmds)| ssb_rom::mesh::SequenceItem {
                    cmds,
                    world: p.world,
                    mobjs: &materials[p.node],
                    mat_anims: &[],
                    depth_seed: ground_layer1_list1_depth_seed(initial, p.list_id),
                    stream: u8::from(p.list_id == Some(1)),
                })
                .collect();

            for (p, converted) in plan.iter().zip(ssb_rom::mesh::convert_sequence(
                &items,
                ssb_rom::mesh::Source::of(file),
                initial,
            )) {
                let key: String = match converted {
                    _ if p.dl == NO_LIST => "no list on this side of the matrix".into(),
                    Err(e) => format!("convert failed: {e:?}"),
                    Ok(m) if m.triangle_count() == 0 => "converted, no triangles".into(),
                    Ok(_) => "converted with triangles".into(),
                };
                if why && !key.starts_with("converted with") {
                    println!("  WHY file {id} node {} -> 0x{:X}: {key}", p.node, p.dl);
                }
                *outcome.entry(key).or_default() += 1;
            }
        }
    }
    println!("\nUnion member each node's `dl` turned out to be:");
    for (k, n) in &members {
        println!("  {n:5}  {k}");
    }

    // `id & 0xF000` picks a matrix kind, and kinds 45-50 rebuild the MVP from
    // the camera basis instead of the node's own rotation — they are
    // billboards. Nothing applies them yet, so these nodes bake to a plain TRS
    // and face wherever their geometry happens to (RE-048).
    println!("\nNode transform kinds (`DObjDesc.id & 0xF000`):");
    for (kind, n) in &kinds {
        println!("  {n:5}  {kind}");
    }

    let resolved: usize = outcome.values().sum();
    println!("\nNode display lists ({resolved} after union resolution):");
    for (k, n) in &outcome {
        println!("  {n:5}  {k}");
    }

    let Some(expect_path) = expect else {
        return Ok(());
    };

    let text = fs::read_to_string(&expect_path)?;
    let (mut matched, mut wrong_len, mut missing) = (0usize, 0usize, 0usize);
    let mut expected_total = 0usize;

    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split('\t').collect();
        let (file, offset, entries) = (
            cols[0].parse::<u32>()?,
            cols[1].parse::<u32>()?,
            cols[2].parse::<usize>()?,
        );
        let name = cols.get(3).copied().unwrap_or("");
        expected_total += 1;

        // The decomp counts the terminator as an entry; we do not.
        let want_nodes = entries - 1;
        match found.get(&file).and_then(|g| g.get(&offset)) {
            Some(&got) if got == want_nodes => matched += 1,
            Some(&got) => {
                wrong_len += 1;
                println!("  LEN  file {file} @ 0x{offset:X} {name}: got {got}, want {want_nodes}");
            }
            None => {
                missing += 1;
                println!("  MISS file {file} @ 0x{offset:X} {name}: {want_nodes} nodes");
            }
        }
    }

    // Everything we found that the decomp has not typed. These are not errors:
    // only 96 of the 2132 files have had their DObjDesc arrays annotated, so
    // most extras are real arrays nobody has labelled yet. The number is worth
    // watching for sudden growth, which would mean the filters loosened.
    let extra = graph_count - matched - wrong_len;

    println!();
    println!(
        "Against {expected_total} annotated arrays in {}:",
        expect_path.display()
    );
    println!("  exact match:   {matched}");
    println!("  wrong length:  {wrong_len}");
    println!("  not found:     {missing}");
    println!("  unannotated:   {extra}");

    if missing > 0 || wrong_len > 0 {
        return Err(format!(
            "{} annotated arrays did not round-trip",
            missing + wrong_len
        )
        .into());
    }
    Ok(())
}

fn mesh(path: &Path) -> Res {
    use ssb_rom::mesh;

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let files: Vec<_> = (0..archive.len() as u32)
        .filter_map(|id| archive.load(id).ok())
        .collect();
    let mut converted = 0usize;
    let mut failed: BTreeMap<String, usize> = BTreeMap::new();
    let mut index_refs = 0usize; // triangle corners
    let mut uniq_vertices = 0usize; // after dedup
    let mut triangles = 0usize;
    let mut draws_after = 0usize; // after merging
    let mut textured = 0usize;
    // Does a vertex's colour field actually hold a unit normal? N64 normals are
    // i8 components of a unit vector, so x^2+y^2+z^2 lands near 127^2 = 16129.
    // Colours have no reason to. This distinguishes "the geometry mode said
    // lit" from "the data is normals", which is the question that matters.
    let mut lit_verts = 0usize;
    let mut unlit_verts = 0usize;
    let mut lit_normal_like = 0usize;
    let mut unlit_normal_like = 0usize;
    let normal_like = |c: [u8; 4]| {
        let x = c[0] as i8 as i32;
        let y = c[1] as i8 as i32;
        let z = c[2] as i8 as i32;
        let m = x * x + y * y + z * z;
        (11000..=21000).contains(&m)
    };

    for file in &files {
        let all = ssb_rom::scan::find_root_display_lists(file);

        // convert() inlines G_DL callees, so a discovered list that another
        // discovered list calls would be counted twice. Keep only true roots.
        let called: std::collections::BTreeSet<u32> =
            all.iter().flat_map(|d| d.referenced_lists()).collect();

        for dl in all.iter().filter(|d| !called.contains(&d.offset)) {
            match mesh::convert(&dl.commands, mesh::Source::of(file)) {
                Ok(m) => {
                    for prim in &m.primitives {
                        let mut idx: Vec<u16> = prim.indices.clone();
                        idx.sort_unstable();
                        idx.dedup();
                        for i in idx {
                            let Some(v) = m.vertices.get(i as usize) else {
                                continue;
                            };
                            let nl = normal_like(v.rgba);
                            if prim.material.lit {
                                lit_verts += 1;
                                lit_normal_like += nl as usize;
                            } else {
                                unlit_verts += 1;
                                unlit_normal_like += nl as usize;
                            }
                        }
                    }
                    converted += 1;
                    uniq_vertices += m.vertex_count();
                    triangles += m.triangle_count();
                    index_refs += m.triangle_count() * 3;
                    draws_after += m.primitives.len();
                    textured += m
                        .primitives
                        .iter()
                        .filter(|p| p.material.texture.is_some())
                        .count();
                }
                Err(e) => *failed.entry(format!("{e:?}")).or_default() += 1,
            }
        }
    }

    println!("normal-vs-colour analysis");
    println!("  vertices in lit prims    {lit_verts}");
    println!("  vertices in unlit prims  {unlit_verts}");
    println!(
        "  unlit that look like unit normals: {unlit_normal_like} ({:.1}%)",
        unlit_normal_like as f64 / unlit_verts.max(1) as f64 * 100.0
    );
    println!(
        "  lit   that look like unit normals: {lit_normal_like} ({:.1}%)",
        lit_normal_like as f64 / lit_verts.max(1) as f64 * 100.0
    );

    println!("\nmesh conversion");
    println!("  display lists converted  {converted}");
    println!(
        "  failed                   {}",
        failed.values().sum::<usize>()
    );
    for (kind, n) in &failed {
        println!("    {kind:<40} {n}");
    }
    println!("  triangles                {triangles}");
    println!("  triangle corners         {index_refs}");
    println!("  unique vertices          {uniq_vertices}");
    if uniq_vertices > 0 {
        println!(
            "  vertex reuse             {:.2}x",
            index_refs as f64 / uniq_vertices as f64
        );
    }
    println!("  draw calls after merge   {draws_after}");
    println!("  textured draws           {textured}");

    // Geometry memory, comparing the three representations that matter.
    // PSP vertex = 12 bytes at 16-bit components; 24 bytes with float pos/uv.
    let soup_float = index_refs * 24; // expanded triangles, float vertices
    let soup_16 = index_refs * 12; // expanded triangles, 16-bit vertices
    let indexed_16 = uniq_vertices * 12 + index_refs * 2; // + u16 indices
    let kib = |b: usize| b as f64 / 1024.0;
    println!("\ngeometry memory");
    println!("  triangle soup, float     {:>8.1} KiB", kib(soup_float));
    println!("  triangle soup, 16-bit    {:>8.1} KiB", kib(soup_16));
    println!("  indexed, 16-bit          {:>8.1} KiB", kib(indexed_16));
    println!(
        "  saving vs float soup     {:>8.1}%",
        100.0 - (indexed_16 as f64 / soup_float as f64 * 100.0)
    );

    Ok(())
}

/// One display list a scene graph draws, and the space it draws it in.
struct PlannedList {
    /// Index of the owning node within its graph.
    node: usize,
    /// File-relative offset of the display list.
    dl: u32,
    /// Modelview in effect while the list runs: the index of the node whose
    /// matrix is current, or `None` for the object root.
    ///
    /// Usually the node's own. A `Gfx *dls[2]` pair's first list draws *before*
    /// the node's matrix is pushed, so it runs in the parent's space instead.
    space: Option<usize>,
    world: ssb_rom::scene::Mat4,
    /// Which task-list command head (`gSYTaskmanDLHeads[N]`) a `DObjDLLink`
    /// entry targets (RE-245/RE-250); `None` for a `Direct`/`Pair` list, which
    /// has no such concept -- `gcDrawDObjTreeForGObj`'s own tree walk always
    /// runs on list 0.
    list_id: Option<u32>,
}

impl PlannedList {
    /// Whether this list can be placed on the node itself. A node holds one
    /// mesh, so anything else has to become an extra leaf; see
    /// `PackWriter::add_object`.
    fn own_space(&self) -> bool {
        self.space == Some(self.node)
    }
}

/// A node with no display list still occupies a slot in the draw sequence, so
/// that vertex-cache state lines up with the game's own walk.
const NO_LIST: u32 = u32::MAX;

/// Flattens a graph into the order `gcDrawDObjTree*` would draw it.
///
/// That order is simply the `DObjDesc` array order: `gcAddChildForDObj` appends
/// each new node to the tail of its parent's sibling list, and the draw walk is
/// node-then-child-then-siblings, so the pre-order flattening the array already
/// is round-trips exactly. Nothing needs sorting.
fn plan_draw_order(
    graph: &ssb_rom::scene::SceneGraph,
    resolver: &ssb_rom::scene::DlResolver,
) -> Vec<PlannedList> {
    use ssb_rom::scene::{Mat4, NodeDl};

    let worlds = graph.world_transforms();
    let mut out = Vec::with_capacity(graph.nodes.len());

    for (i, node) in graph.nodes.iter().enumerate() {
        let Some(node_dl) = node.desc.dl else {
            continue;
        };
        let own = |dl, list_id| PlannedList {
            node: i,
            dl,
            space: Some(i),
            world: worlds[i],
            list_id,
        };
        match resolver.resolve(node_dl) {
            NodeDl::Direct(dl) => out.push(own(dl, None)),
            NodeDl::Links(links) => out.extend(
                links
                    .iter()
                    .filter_map(|l| l.dl.map(|dl| own(dl, Some(l.list_id)))),
            ),
            NodeDl::Pair { pre, post } => {
                if let Some(dl) = pre {
                    out.push(PlannedList {
                        node: i,
                        dl,
                        space: node.parent,
                        world: node.parent.map_or(Mat4::IDENTITY, |p| worlds[p]),
                        list_id: None,
                    });
                }
                // The node's matrix is pushed between the two, so even when
                // `post` is NULL the node still occupies a step in the walk.
                out.push(own(post.unwrap_or(NO_LIST), None));
            }
        }
    }
    out
}

/// Joint entries in a built pack that name both a script and a node.
///
/// The gap between this and the total is the two ways a joint can be inert: an
/// animation that does not move it, and a spare runtime-joint entry a table
/// carries for one of `TransN`, `XRotN`, or `YRotN` (RE-036).
fn pack_anim_joints_bound(pack: &ssb_rom::pack::Pack<'_>) -> usize {
    (0..pack.anim_joint_count())
        .filter_map(|i| pack.anim_joint(i))
        .filter(|j| {
            j.script != ssb_rom::pack::AnimJoint::NO_SCRIPT
                && j.node != ssb_rom::pack::AnimJoint::NO_NODE
        })
        .count()
}

/// Where a primitive's texels may be found.
///
/// Two files rather than one, because a texture need not live in the file that
/// draws it: every stage reaches its texels through a pointer into a separate
/// file (RE-037).
#[derive(Clone, Copy)]
struct Texels<'a> {
    /// The file the display list came from.
    home: &'a ssb_rom::archive::File,
    /// The whole archive, for a reference that names another file.
    all: &'a [Option<ssb_rom::archive::File>],
}

impl<'a> Texels<'a> {
    /// The bytes of the file a reference's half names, or of `home`.
    fn bytes(&self, which: Option<u16>) -> Option<&'a [u8]> {
        match which {
            None => Some(&self.home.data[..]),
            Some(id) => self.all.get(id as usize)?.as_ref().map(|f| &f.data[..]),
        }
    }
}

/// Adds a converted mesh to the pack, uploading any textures it samples.
///
/// `files` is the whole archive because a texture need not live in the file
/// that draws it: every stage reaches its texels through a pointer into a
/// separate file (RE-037). The cache key is therefore the file the *texels*
/// are in, not the one the display list came from — otherwise the four stages
/// sharing a texture file would each upload their own copy of it.
///
/// `mat_anim_data` is looked up whenever a primitive's material carries a
/// [`ssb_rom::mesh::MatAnimRef`] (RE-091): converts every resolved palette
/// variant the same way [`convert_texture`] converts the static one, and
/// points the texture at the resulting [`ssb_rom::pack::MatAnimDesc`] --
/// deduplicated via `mat_anim_index` the same way textures already are, so
/// many primitives sharing one script do not upload its palettes twice.
/// Keys the texture cache on every [`ssb_rom::mesh::TextureRef`] field
/// [`convert_texture`] actually reads to produce its baked bytes (RE-253),
/// plus the texel/palette file each `Option<u16>` resolves to. R0.16/RE-122
/// already moved this key from a bare `(image_file, image_offset,
/// palette_file, palette_offset)` 4-tuple to an 8-tuple adding `mirror_s`/
/// `mirror_t`/`clamp_s`/`clamp_t`, on exactly this reasoning -- state
/// `convert_texture` bakes into different bytes must be in the key, or
/// whichever primitive converts first silently donates its own bytes to
/// every other primitive sharing the narrower key.
///
/// RE-253 measured the 8-tuple still missing every other field
/// `convert_texture` reads: `width`/`height` (the loaded/cropped tile size),
/// `drawn_width`/`drawn_height` (the mirror-bake rect, RE-067), `format`/
/// `size`, and `palette_entries`/`palette` (the CI4 bank, RE-223). Two
/// primitives sharing one base image+palette+wrap but drawing a *different*
/// crop of it (an ordinary pattern for a shared character texture sheet)
/// silently shared one cache entry. Found while investigating why RE-252's
/// submission-order fix changed 11 of 15 golden scenes far more than its own
/// primitive-reorder effect could explain -- the reorder was simply picking
/// a different arbitrary "first" among already-colliding keys, not
/// introducing a new problem.
///
/// `format`/`size`/`palette_entries`/`palette` were first assumed safe to
/// leave out, on the theory that reading the same ROM address always decodes
/// it the same way -- an assumption, not yet a measurement, so
/// `texture_key_fields_never_vary_for_a_fixed_data_and_palette_location`
/// below was written to check it before shipping that gap. It measured the
/// assumption **false**: 1 real `(data_file, data_offset)` pair with two
/// different format/size, and 46 real `(palette_file, palette_offset)` pairs
/// with two different `palette_entries`/bank values, archive-wide. All four
/// fields are in the key below.
///
/// A first attempt keyed on the *entire* `TextureRef` instead of this
/// specific field set, reasoning from `merge_by_material`'s own whole-struct
/// `MeshMaterial` key (RE-252). That overcorrected: `origin_s`/`origin_t`/
/// `mask_s`/`mask_t`/`framebuffer` are real `TextureRef` fields but
/// `convert_texture` never reads any of them (they are draw-time UV/clamp
/// inputs consumed elsewhere, not baking inputs) — including them fragmented
/// legitimately-identical baked textures across every primitive with a
/// merely different absolute tile origin, inflating the archive-wide texture
/// count from 1,345 to 2,011 (+49.5%) instead of closing RE-253's measured
/// 45-texture gap. `MeshMaterial`'s whole-struct key is safe because every
/// one of its fields *is* real render state; `TextureRef` mixes baking
/// inputs with draw-time-only ones, so the correct key is the exhaustive
/// subset `convert_texture` depends on, not the whole struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TexKey {
    data_file: u32,
    data_offset: u32,
    palette_file: u32,
    palette_offset: Option<u32>,
    format: ssb_rom::texture::Format,
    size: ssb_rom::texture::BitSize,
    width: u16,
    height: u16,
    source_width: u16,
    drawn_width: u16,
    drawn_height: u16,
    palette_entries: u16,
    palette: u8,
    /// `G_MDSFT_TEXTLUT` changes what the same bytes decode to (RE-313).
    tlut: ssb_rom::texture::TextureLut,
    mirror_s: bool,
    mirror_t: bool,
    clamp_s: bool,
    clamp_t: bool,
    compensation_version: u8,
    coverage_hash: u64,
    animated_relationship: bool,
    animated_source: Option<ssb_rom::mesh::MatAnimRef>,
    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy,
    /// Exact RE-312 dense variant; zero is the ordinary conversion.
    residual_variant: u16,
}

#[derive(Default)]
struct MatAnimData {
    source_offset: u32,
    palette_entries: u16,
    palettes: Vec<ssb_rom::mobj::Ptr>,
    sprites: Vec<ssb_rom::mobj::Ptr>,
    base_tracks: [u32; 10],
    uv_mode: u8,
    uv_tile_params: [u16; 3],
    /// RE-327: `unk24`/`unk28`/`unk44` bits for `unk10 == 1`.
    uv_half: [u32; 3],
    /// RE-318: the script can never move `TraU`/`TraV`/`ScaU`/`ScaV` off
    /// their rest values, so the primitive samples exactly its authored UVs.
    uv_static: bool,
    /// RE-321: the script drives `SetLFrac` or `TextureIDNext`, the two
    /// inputs of the two-tile fractional blend.
    drives_lod: bool,
    /// RE-321: the highest `sprites[]` index `TextureIDCurrent` reaches in the
    /// replay (0 when the track never moves: `gcAddMObjForDObj` zeroes it).
    /// `sprites` also covers `TextureIDNext`, which only tile 1 samples.
    max_current: usize,
}

fn texture_cache_key(
    id: u32,
    t: &ssb_rom::mesh::TextureRef,
    coverage_hash: u64,
    animated_relationship: Option<ssb_rom::mesh::MatAnimRef>,
    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy,
) -> TexKey {
    TexKey {
        data_file: t.data_file.map_or(id, u32::from),
        data_offset: t.data_offset,
        palette_file: t.palette_file.map_or(id, u32::from),
        palette_offset: t.palette_offset,
        format: t.format,
        size: t.size,
        width: t.width,
        height: t.height,
        source_width: t.source_width,
        drawn_width: t.drawn_width,
        drawn_height: t.drawn_height,
        palette_entries: t.palette_entries,
        palette: t.palette,
        tlut: t.tlut,
        mirror_s: t.mirror_s,
        mirror_t: t.mirror_t,
        clamp_s: t.clamp_s,
        clamp_t: t.clamp_t,
        compensation_version: ssb_rom::filter_compensation::ALGORITHM_VERSION,
        coverage_hash,
        animated_relationship: animated_relationship.is_some(),
        animated_source: animated_relationship,
        alpha_policy,
        residual_variant: 0,
    }
}

fn primitive_filter_coverage(
    m: &ssb_rom::mesh::Mesh,
    prim: &ssb_rom::mesh::Primitive,
) -> filter_coverage::Coverage {
    // Authored UVs: the N=8 barycentric base plus bounded, cell-local filter
    // boundaries and a separate denser holdout.
    if let Some(scale) = prim.material.texgen_scale {
        return texgen_filter_coverage(m, prim, scale);
    }
    let triangles: Vec<_> = prim
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|tri| [tri[0], tri[1], tri[2]].map(|i| m.vertices[i as usize].uv.map(i32::from)))
        .collect();
    filter_coverage::build(&triangles)
}

/// Texgen coverage from the primitive's real normals (RE-311).
///
/// Generated coordinates depend on the live look-at basis and node
/// transform. Neither is statically enumerable: `gmCameraLookAtFuncMatrix`
/// rebuilds the look-at from the camera's live eye/at every frame, and a
/// fighter's joint animation rotates the node freely. The real normals, the
/// `/127` dot, `gSPTexture` scale, tile origin/clamp and S10.5 truncation
/// are all static, so coverage runs them through an orientation quadrature
/// (`filter_coverage::build_texgen`) and bounds every pose by
/// `filter_coverage::TexgenBound`.
///
/// Falls back to the pre-RE-311 full-tile coverage when the generated
/// footprint cannot be bounded from static data: an animated material may
/// apply a live UV affine (`MaterialUv`) on top of the generated coordinates.
fn texgen_filter_coverage(
    m: &ssb_rom::mesh::Mesh,
    prim: &ssb_rom::mesh::Primitive,
    scale: (u16, u16),
) -> filter_coverage::Coverage {
    let Some(t) = prim.material.texture else {
        return filter_coverage::Coverage::default();
    };
    if prim.material.mat_anim.is_some() || prim.indices.is_empty() {
        return filter_coverage::Coverage::default();
    }
    let normals: Vec<[[i8; 3]; 3]> = prim
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|tri| {
            [tri[0], tri[1], tri[2]].map(|i| {
                let rgba = m.vertices[i as usize].rgba;
                [rgba[0] as i8, rgba[1] as i8, rgba[2] as i8]
            })
        })
        .collect();
    let input = filter_coverage::TexgenPrimitive {
        linear: prim.material.texture_gen == ssb_rom::mesh::TextureGen::Linear,
        normals: &normals,
        scale: [scale.0, scale.1],
        origin: [t.origin_s, t.origin_t],
        clamp: [t.clamp_s, t.clamp_t],
    };
    let coverage = filter_coverage::build_texgen(&input);
    let mode = std::env::var("ROMTOOL_TEXGEN_COVERAGE").unwrap_or_default();
    if mode == "full-tile" || mode == "full-tile-per-primitive" {
        // Measurement only: reproduce the pre-RE-311 optimisation and gate
        // exactly, keeping the real sets for the report line. `full-tile`
        // reproduces the pre-RE-311 pack byte for byte.
        return filter_coverage::Coverage {
            train: Vec::new(),
            validation: Vec::new(),
            texgen: coverage.texgen.map(|meta| filter_coverage::TexgenMeta {
                legacy: true,
                per_primitive: mode == "full-tile-per-primitive",
                ..meta
            }),
        };
    }
    coverage
}

fn coverage_hash(coverage: &[[i32; 2]]) -> u64 {
    coverage
        .iter()
        .flatten()
        .fold(0xcbf29ce484222325u64, |h, &v| {
            (h ^ v as u64).wrapping_mul(0x100000001b3)
        })
}

fn validation_preserved(
    baseline: ssb_rom::filter_compensation::Metrics,
    candidate: ssb_rom::filter_compensation::Metrics,
    policy: ssb_rom::filter_compensation::AlphaPolicy,
) -> bool {
    baseline.samples > 0
        && candidate.samples == baseline.samples
        && candidate.squared_error <= baseline.squared_error
        && candidate.above_8 <= baseline.above_8
        && candidate.above_32 <= baseline.above_32
        && candidate.max <= baseline.max.saturating_add(8)
        && (policy != ssb_rom::filter_compensation::AlphaPolicy::Translucent
            || candidate.visible_squared_error <= baseline.visible_squared_error)
}

/// `--hide-wide-tiles`: see its option comment in [`pack`].
static HIDE_WIDE_TILES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

thread_local! {
    /// RE-318 census for the pack summary: source primitives lowered,
    /// triangles split, primitives emitted.
    static WIDE_TILE_STATS: std::cell::RefCell<(usize, usize, usize)> = const { std::cell::RefCell::new((0, 0, 0)) };
    /// RE-321 census for the pack summary: every primitive the converter
    /// classified as a two-tile blend, with the outcome.
    static LOD_BLEND_SITES: std::cell::RefCell<Vec<LodBlendSite>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// One classified two-tile blend primitive and what the packer did with it.
struct LodBlendSite {
    file: u32,
    dl: u32,
    prim: usize,
    triangles: usize,
    /// `Ok(next textures added)` or the reason the primitive keeps its
    /// single-texture path.
    outcome: Result<usize, &'static str>,
}

/// RE-321: whether every texel `t` can sample is fully opaque, read from the
/// source. Only CI through a TLUT is measured; anything else declines.
fn source_fully_opaque(src: Texels<'_>, t: &ssb_rom::mesh::TextureRef) -> Option<bool> {
    use ssb_rom::texture;
    if !t.tlut.enabled() || t.format != texture::Format::Ci {
        return None;
    }
    let file = src.bytes(t.data_file)?;
    let indices = decode_index_field(file, t)?;
    let bank = if t.size == texture::BitSize::Bits4 {
        t.palette
    } else {
        0
    };
    let (off, n) = palette_bank_offset(t.palette_offset?, t.palette_entries, bank);
    let tlut = src
        .bytes(t.palette_file)?
        .get(off as usize..off as usize + n * 2)
        .map(texture::parse_tlut)?;
    Some(indices.iter().all(|&i| {
        tlut.get(usize::from(i))
            .is_some_and(|&e| t.tlut.entry_rgba(e)[3] == 255)
    }))
}

/// RE-321: converts a two-tile blend primitive's `TEXEL1` images and checks
/// that the two-pass GE lowering reproduces its RDP output, returning the
/// record to pack or the reason to keep the single-texture path.
///
/// The lowering is exact when pass 1's pixel replaces the framebuffer and the
/// alpha gate cannot tell pass 1's `TEXEL0` alpha from the blended alpha:
///
/// * no GE blending (`translucent` with a classified alpha formula);
/// * no gate, or a zero-threshold gate with every `TEXEL0` image opaque and
///   `PRIM_ALPHA` 255 -- then the blended alpha is at least `1 - f` with
///   `f <= 255/256`, which never fails a zero threshold, and pass 1's alpha
///   is 255;
/// * no palette animation and no texgen, which the second pass does not
///   carry.
///
/// `TEXEL1` images convert through the ordinary texture pipeline, shaped by
/// tile 1's own descriptor, under the opaque alpha policy: pass 2 blends by
/// fixed factors, and the RDP interpolates `TEXEL1`'s colour even where its
/// alpha is zero, so that colour must survive compensation.
#[allow(clippy::too_many_arguments)]
fn lod_blend_desc(
    writer: &mut ssb_rom::pack::PackWriter,
    src: Texels<'_>,
    id: u32,
    offset: u32,
    prim_index: usize,
    m: &ssb_rom::mesh::Mesh,
    prim: &ssb_rom::mesh::Primitive,
    lod: &ssb_rom::mesh::LodBlend,
    mat_anim: u32,
    data: &MatAnimData,
    swizzle: bool,
    phase: [i16; 2],
) -> Result<ssb_rom::pack::LodBlendDesc, &'static str> {
    use ssb_rom::pack::AlphaGate;
    if !data.drives_lod {
        return Err("script drives neither SetLFrac nor TextureIDNext");
    }
    if data.sprites.is_empty() {
        return Err("no sprite table for TextureIDNext");
    }
    if !data.palettes.is_empty() {
        return Err("PaletteID animation is not carried by the second pass");
    }
    if prim.material.texture_gen != ssb_rom::mesh::TextureGen::None {
        return Err("texgen is not carried by the second pass");
    }
    let base = prim.material.texture.ok_or("no TEXEL0 texture")?;
    let (gate, translucent_blend) = ssb_rom::pack::material_alpha_state(&prim.material);
    if translucent_blend {
        return Err("GE blending would weight pass 1 by its own alpha");
    }
    let zero_gate = match gate {
        AlphaGate::Off => false,
        AlphaGate::Greater(0) | AlphaGate::GreaterOrEqual(0 | 1) => true,
        _ => return Err("nonzero alpha threshold"),
    };
    if zero_gate {
        if lod.prim_alpha != 255 {
            return Err("PRIM_ALPHA below 255 under an alpha gate");
        }
        let reached = data.sprites.iter().take(data.max_current + 1);
        let current = std::iter::once(base).chain(reached.map(|p| ssb_rom::mesh::TextureRef {
            data_file: p.file,
            data_offset: p.offset,
            ..base
        }));
        for t in current {
            if source_fully_opaque(src, &t) != Some(true) {
                return Err("a TEXEL0 image is not provably opaque under an alpha gate");
            }
        }
    }
    let coverage = primitive_filter_coverage(m, prim);
    let policy = ssb_rom::filter_compensation::AlphaPolicy::Opaque;
    let mut next_textures =
        [ssb_rom::pack::TextureDesc::NO_ANIM; ssb_rom::pack::MatAnimDesc::MAX_TEXTURES];
    for (slot, p) in data.sprites.iter().enumerate() {
        residuals::discard_conversion();
        let tex = convert_mat_anim_sprite(src, *p, &lod.next, &coverage, None, swizzle, policy)
            .ok_or("a TEXEL1 image did not convert")?;
        let i = writer.add_texture(&tex, lod.next.clamp_s, lod.next.clamp_t);
        writer.set_texture_tile(i, &lod.next);
        residuals::record_variant(i, &tex, lod.next.clamp_s, lod.next.clamp_t);
        if residuals::enabled() {
            residuals::record_site(residuals::UseSite {
                variant: i,
                file: id,
                dl: offset,
                prim: prim_index,
                sprite_slot: Some(slot),
                policy,
                texgen: None,
                mat_anim: true,
                triangles: prim.triangle_count(),
                coverage: coverage.clone(),
                phase,
            });
        }
        next_textures[slot] = i;
    }
    Ok(ssb_rom::pack::LodBlendDesc {
        mat_anim,
        next_count: data.sprites.len() as u32,
        next_textures,
        tile1_params: lod.tile1_params.map(u32::from),
    })
}

#[allow(clippy::too_many_arguments)]
fn pack_mesh(
    writer: &mut ssb_rom::pack::PackWriter,
    tex_index: &mut BTreeMap<TexKey, u32>,
    mat_anim_index: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, u32>,
    mat_anim_data: &BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
    src: Texels<'_>,
    id: u32,
    offset: u32,
    source: &ssb_rom::mesh::Mesh,
    swizzle: bool,
) -> u32 {
    // RE-318: split tiles whose bake would exceed the GE's 512-texel limit
    // into equivalent repeat/window tiles before anything is converted.
    // `source_prim` keeps residual fixes and report sites keyed by the
    // display list's own primitive order.
    let lowered = ssb_rom::wide_tile::lower(source, |i| {
        source.primitives[i]
            .material
            .mat_anim
            .is_none_or(|a| mat_anim_data.get(&a).is_none_or(|d| d.uv_static))
    })
    .unwrap_or_else(|e| panic!("RE-318 wide tile in file {id} list 0x{offset:X}: {e:?}"));
    if lowered.stats.primitives > 0 {
        WIDE_TILE_STATS.with(|s| {
            let mut s = s.borrow_mut();
            s.0 += lowered.stats.primitives;
            s.1 += lowered.stats.split_triangles;
            s.2 += lowered.stats.emitted;
        });
    }
    let mut lowered = lowered;
    if HIDE_WIDE_TILES.get().copied().unwrap_or(false) {
        for (k, prim) in lowered.mesh.primitives.iter_mut().enumerate() {
            // A lowered piece always differs from its source in texture.
            if prim.material.texture != source.primitives[lowered.source_prim[k]].material.texture {
                prim.indices.clear();
            }
        }
    }
    let m = &lowered.mesh;
    let mut per_prim: Vec<Option<u32>> = Vec::with_capacity(m.primitives.len());
    let mut per_prim_mat_anim: Vec<Option<u32>> = Vec::with_capacity(m.primitives.len());
    let mut per_prim_phase = Vec::with_capacity(m.primitives.len());
    let mut per_prim_lod = Vec::with_capacity(m.primitives.len());
    for (packed_index, prim) in m.primitives.iter().enumerate() {
        let prim_index = lowered.source_prim[packed_index];
        let fix = prim
            .material
            .texture
            .as_ref()
            .and_then(|t| residual_fixes::find(id, offset, prim_index, t));
        let phase = match fix.map(|f| f.kind) {
            Some(residual_fixes::Kind::Phase(p)) => p,
            _ => [0, 0],
        };
        per_prim_phase.push(phase);
        let texgen_site = prim
            .material
            .texgen_scale
            .map(|_| prim.material.texture_gen == ssb_rom::mesh::TextureGen::Linear);
        let texture_index = match prim.material.texture {
            None => None,
            // RE-099/RE-100: no ROM bytes to convert -- the device fills
            // this in at run time (`Gpu::request_transition_capture`). Keyed
            // on width/height alone, under a `(u32::MAX, u32::MAX)` file
            // prefix no real resolved file id can ever equal: unlike a real
            // texture, a framebuffer capture has no baked crop/mirror bytes
            // to disambiguate (there is no pixel data to bake, RE-253), so
            // every other field is pinned to a fixed sentinel value rather
            // than the real primitive's own -- otherwise this would fragment
            // one runtime capture into needless duplicate pack entries
            // across primitives that legitimately share it at different tile
            // origins.
            Some(t) if t.framebuffer => {
                let key = TexKey {
                    data_file: u32::MAX,
                    data_offset: 0,
                    palette_file: u32::MAX,
                    palette_offset: None,
                    format: ssb_rom::texture::Format::Rgba,
                    size: ssb_rom::texture::BitSize::Bits4,
                    width: t.width,
                    height: t.height,
                    source_width: t.width,
                    drawn_width: 0,
                    drawn_height: 0,
                    palette_entries: 0,
                    palette: 0,
                    tlut: ssb_rom::texture::TextureLut::None,
                    mirror_s: false,
                    mirror_t: false,
                    clamp_s: false,
                    clamp_t: false,
                    compensation_version: 0,
                    coverage_hash: 0,
                    animated_relationship: false,
                    animated_source: None,
                    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy::Opaque,
                    residual_variant: 0,
                };
                Some(
                    *tex_index
                        .entry(key)
                        .or_insert_with(|| writer.add_framebuffer_texture(t.width, t.height)),
                )
            }
            Some(t) => {
                let original_coverage = primitive_filter_coverage(m, prim);
                let coverage = if matches!(fix.map(|f| f.kind), Some(residual_fixes::Kind::Dense)) {
                    residual_fixes::dense_coverage(&original_coverage)
                } else {
                    original_coverage
                };
                let animated = prim.material.mat_anim;
                let (gate, translucent_blend) = ssb_rom::pack::material_alpha_state(&prim.material);
                let alpha_policy =
                    ssb_rom::filter_compensation::AlphaPolicy::classify(gate, translucent_blend);
                let mut key = texture_cache_key(
                    id,
                    &t,
                    coverage_hash(&coverage.train)
                        ^ coverage_hash(&coverage.validation).rotate_left(1)
                        ^ coverage
                            .texgen
                            .as_ref()
                            .filter(|meta| meta.per_primitive)
                            .map_or(0, |meta| coverage_hash(&meta.holdout).rotate_left(2)),
                    animated,
                    alpha_policy,
                );
                let direct_fit = match fix.map(|f| f.kind) {
                    Some(residual_fixes::Kind::Direct(mode)) => Some(mode),
                    _ => None,
                };
                if matches!(
                    fix.map(|f| f.kind),
                    Some(residual_fixes::Kind::Dense | residual_fixes::Kind::Direct(_))
                ) {
                    key.residual_variant = fix.unwrap().variant;
                }
                let resolved = if let Some(&i) = tex_index.get(&key) {
                    Some(i)
                } else {
                    residuals::discard_conversion();
                    let mut compensated = false;
                    let animated_palettes = animated.and_then(|key| {
                        mat_anim_data
                            .get(&key)
                            .and_then(|data| reachable_palettes(src, key, data, None))
                    });
                    convert_texture(
                        src,
                        &t,
                        &coverage,
                        swizzle,
                        animated_palettes.as_deref(),
                        alpha_policy,
                        Some(&mut compensated),
                        direct_fit,
                    )
                    .map(|tex| {
                        let final_key = if compensated {
                            key
                        } else {
                            let mut key = texture_cache_key(
                                id,
                                &t,
                                0,
                                animated,
                                ssb_rom::filter_compensation::AlphaPolicy::Opaque,
                            );
                            key.animated_source = None;
                            key.residual_variant = 0;
                            key
                        };
                        if let Some(&i) = tex_index.get(&final_key) {
                            residuals::discard_conversion();
                            return i;
                        }
                        let i = writer.add_texture(&tex, t.clamp_s, t.clamp_t);
                        writer.set_texture_tile(i, &t);
                        residuals::record_variant(i, &tex, t.clamp_s, t.clamp_t);
                        tex_index.insert(final_key, i);
                        i
                    })
                };
                // Report-only (`romtool residuals`): every primitive that
                // binds a variant, with its real filter coverage.
                if let (Some(i), true) = (resolved, residuals::enabled()) {
                    residuals::record_site(residuals::UseSite {
                        variant: i,
                        file: id,
                        dl: offset,
                        prim: prim_index,
                        sprite_slot: None,
                        policy: alpha_policy,
                        texgen: texgen_site,
                        mat_anim: animated.is_some(),
                        triangles: prim.triangle_count(),
                        coverage,
                        phase,
                    });
                }
                resolved
            }
        };
        // Unlike `texture_index`, not gated on the primitive having a bound
        // texture at all: an effect script can drive untextured primitive/
        // environment/blend colour with no palette or sprite involved
        // (RE-175, `mesh.rs`'s `apply_mobj`/`material_now`).
        let mat_anim_index_resolved = prim.material.mat_anim.and_then(|anim| {
            let key = anim;
            match mat_anim_index.get(&key) {
                Some(&i) => Some(i),
                None => mat_anim_data.get(&key).and_then(|anim_data| {
                    let file_bytes = src.bytes(if anim.source_file == id {
                        None
                    } else {
                        Some(anim.source_file as u16)
                    })?;
                    let palettes: Vec<Vec<u32>> = anim_data
                        .palettes
                        .iter()
                        .filter_map(|p| {
                            convert_mat_anim_palette(src, *p, anim_data.palette_entries)
                        })
                        .collect();
                    // A partial conversion is a real problem worth declining
                    // outright, not shipping a script that cycles through
                    // fewer palettes than it actually names.
                    if palettes.len() != anim_data.palettes.len() {
                        return None;
                    }
                    // A texture-id track needs the primitive's own bound
                    // texture to know the sprite's format/dimensions/wrap
                    // (`convert_mat_anim_sprite`) -- an effect script whose
                    // primitive resolved no texture at all cannot be
                    // converted this way, same "decline rather than guess"
                    // shape as the palette check above. `texture_shape`
                    // (RE-177) covers the real gap this left: several
                    // manager-effect sprite primitives issue `G_SETTILE`/
                    // `G_SETTILESIZE` for their sprite's shape but never a
                    // static `G_SETTIMG` for it at all, because real hardware
                    // only supplies that address at runtime through a
                    // graphics-heap `Call` this converter cannot follow --
                    // the shape is real and known even though `texture`
                    // itself correctly stays `None`.
                    let sprites: Vec<u32> = if anim_data.sprites.is_empty() {
                        Vec::new()
                    } else {
                        // RE-326: only a primitive drawing the `MObj`'s own
                        // image has the tile shape its sprites load into. A
                        // later one that loaded its own image leaves the
                        // entry to that primitive.
                        if !prim.material.anim_texture.image {
                            NON_OWNER_FIRST.with(|n| n.set(n.get() + 1));
                            return None;
                        }
                        let base = prim.material.texture.or(prim.material.texture_shape)?;
                        let (gate, translucent_blend) =
                            ssb_rom::pack::material_alpha_state(&prim.material);
                        let alpha_policy = ssb_rom::filter_compensation::AlphaPolicy::classify(
                            gate,
                            translucent_blend,
                        );
                        let coverage = primitive_filter_coverage(m, prim);
                        let converted: Vec<u32> = anim_data
                            .sprites
                            .iter()
                            .enumerate()
                            .filter_map(|(slot, p)| {
                                let states = reachable_palettes(src, anim, anim_data, Some(slot));
                                residuals::discard_conversion();
                                convert_mat_anim_sprite(
                                    src,
                                    *p,
                                    &base,
                                    &coverage,
                                    states.as_deref(),
                                    swizzle,
                                    alpha_policy,
                                )
                                .map(|tex| {
                                    let i = writer.add_texture(&tex, base.clamp_s, base.clamp_t);
                                    writer.set_texture_tile(i, &base);
                                    residuals::record_variant(i, &tex, base.clamp_s, base.clamp_t);
                                    if residuals::enabled() {
                                        residuals::record_site(residuals::UseSite {
                                            variant: i,
                                            file: id,
                                            dl: offset,
                                            prim: prim_index,
                                            sprite_slot: Some(slot),
                                            policy: alpha_policy,
                                            texgen: texgen_site,
                                            mat_anim: true,
                                            triangles: prim.triangle_count(),
                                            coverage: coverage.clone(),
                                            phase,
                                        });
                                    }
                                    i
                                })
                            })
                            .collect();
                        if converted.len() != anim_data.sprites.len() {
                            return None;
                        }
                        converted
                    };
                    let i = writer.add_mat_anim(
                        anim.source_file,
                        file_bytes,
                        anim.script,
                        anim_data.source_offset,
                        &palettes,
                        &sprites,
                        anim_data.base_tracks,
                        anim_data.uv_mode,
                        anim_data.uv_tile_params,
                    );
                    writer.set_mat_anim_uv_half(i, anim_data.uv_half);
                    mat_anim_index.insert(key, i);
                    Some(i)
                }),
            }
        });
        if let (Some(texture), Some(mat_anim), true) = (
            texture_index,
            mat_anim_index_resolved,
            prim.material.anim_texture.palette,
        ) {
            // Kept alongside `PrimDesc.mat_anim` for the texture-only
            // palette-cycling case (RE-089/RE-090/RE-091): `bind_texture`
            // (`psp-asset-viewer/src/meshdraw.rs`) still reads a texture's own resolved
            // palette off `TextureDesc.mat_anim`, independent of which
            // primitive happens to be drawing it.
            writer.set_texture_mat_anim(texture, mat_anim);
        }
        // RE-321: a classified two-tile blend gets its second pass only
        // once the lowering is proven exact for it; otherwise it keeps the
        // single-texture path.
        let lod = prim.material.lod_blend.map(|lod| {
            let outcome = if prim.material.texture != source.primitives[prim_index].material.texture
            {
                Err("split by the wide-tile lowering")
            } else if texture_index.is_none() {
                Err("TEXEL0 did not convert")
            } else {
                match (prim.material.mat_anim, mat_anim_index_resolved) {
                    (Some(key), Some(anim)) => match mat_anim_data.get(&key) {
                        Some(data) => lod_blend_desc(
                            writer, src, id, offset, prim_index, m, prim, &lod, anim, data,
                            swizzle, phase,
                        )
                        .and_then(|desc| {
                            if writer.add_lod_blend(desc) {
                                Ok(desc.next_count as usize)
                            } else {
                                Err("one script drives two different tile-1 shapes")
                            }
                        }),
                        None => Err("material animation did not resolve"),
                    },
                    _ => Err("material animation did not pack"),
                }
            };
            LOD_BLEND_SITES.with(|s| {
                s.borrow_mut().push(LodBlendSite {
                    file: id,
                    dl: offset,
                    prim: prim_index,
                    triangles: prim.triangle_count(),
                    outcome,
                })
            });
            outcome.is_ok()
        });
        per_prim.push(texture_index);
        per_prim_mat_anim.push(mat_anim_index_resolved);
        per_prim_lod.push(lod == Some(true));
    }
    let mesh_index = writer.add_mesh(m, id, offset, |i| per_prim[i], |i| per_prim_mat_anim[i]);
    for (i, &phase) in per_prim_phase.iter().enumerate() {
        if phase != [0, 0] {
            writer.set_mesh_prim_phase(mesh_index, i, phase);
        }
    }
    for (i, &lod) in per_prim_lod.iter().enumerate() {
        if lod {
            writer.set_mesh_prim_lod_blend(mesh_index, i);
        }
    }
    mesh_index
}

/// Converts one resolved palette variant to the GE's ABGR8888 CLUT format,
/// the same conversion [`convert_texture`] already applies to the static
/// (index-0) palette every texture bakes at pack time.
fn convert_mat_anim_palette(
    src: Texels<'_>,
    p: ssb_rom::mobj::Ptr,
    entries: u16,
) -> Option<Vec<u32>> {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture;

    let n = entries.max(1) as usize;
    let bytes = src
        .bytes(p.file)?
        .get(p.offset as usize..p.offset as usize + n * 2)?;
    let tlut = texture::parse_tlut(bytes);
    Some(
        tlut.iter()
            .map(|&e| psp::pack_abgr(texture::rgba5551(e)))
            .collect(),
    )
}

/// Replay the real script to retain only PaletteID slots that can coincide
/// with this texture selection. `sprite_slot == None` means the primitive's
/// static texture before a TextureIDCurrent track takes over.
fn reachable_palettes(
    src: Texels<'_>,
    key: ssb_rom::mesh::MatAnimRef,
    data: &MatAnimData,
    sprite_slot: Option<usize>,
) -> Option<Vec<Vec<u32>>> {
    if data.palettes.is_empty() {
        return None;
    }
    let file = src.bytes(if key.source_file == src.home.id {
        None
    } else {
        Some(key.source_file as u16)
    })?;
    let mut joint = ssb_rom::matanim::MaterialJoint::start(key.script, 0.0);
    let mut reached = std::collections::BTreeSet::new();
    for _ in 0..MAT_ANIM_REPLAY_FRAMES {
        joint.tick(file, 1.0).ok()?;
        let texture = joint
            .track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_CURRENT)
            .map(|v| (v.max(0.0) as usize).min(data.sprites.len().saturating_sub(1)));
        // `palettes[(s32)palette_id]` for any live kind (RE-325/RE-326).
        if texture == sprite_slot {
            if let Some(value) = joint.track_value(ssb_rom::matanim::TRACK_PALETTE_ID) {
                reached.insert((value.max(0.0) as usize).min(data.palettes.len() - 1));
            }
        }
        if joint.ended() || joint.looped() {
            break;
        }
    }
    reached
        .into_iter()
        .map(|id| convert_mat_anim_palette(src, data.palettes[id], data.palette_entries))
        .collect()
}

/// Converts one resolved sprite (texture-id) variant, reusing a primitive's
/// own authored format/dimensions/wrap (`base`) -- a material animation's
/// `TextureIDCurrent`/`TextureIDNext` track only ever swaps *which* texel
/// block loads, never the tile's shape, the same convention
/// [`convert_mat_anim_palette`] already follows for a palette variant's
/// entry count.
fn convert_mat_anim_sprite(
    src: Texels<'_>,
    p: ssb_rom::mobj::Ptr,
    base: &ssb_rom::mesh::TextureRef,
    coverage: &filter_coverage::Coverage,
    palettes: Option<&[Vec<u32>]>,
    swizzle: bool,
    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy,
) -> Option<ssb_rom::psp_texture::PspTexture> {
    let variant = ssb_rom::mesh::TextureRef {
        data_file: p.file,
        data_offset: p.offset,
        ..*base
    };
    convert_texture(
        src,
        &variant,
        coverage,
        swizzle,
        palettes,
        alpha_policy,
        None,
        None,
    )
}

/// Read the source CI field before colour decoding so equal RGB entries keep
/// their distinct PaletteID meanings. Mirror/clamp expansion follows the
/// same path as `decode_mirrored` in `convert_texture`.
fn decode_index_field(file: &[u8], t: &ssb_rom::mesh::TextureRef) -> Option<Vec<u8>> {
    use ssb_rom::texture::{self, BitSize};
    let source_width = u32::from(t.source_width.max(t.width));
    let width = u32::from(t.width);
    let height = u32::from(t.height);
    let row = texture::data_len(source_width, 1, t.size);
    let bytes = file.get(t.data_offset as usize..t.data_offset as usize + row * height as usize)?;
    let mut img = texture::Rgba8::new(width, height);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let at = y * row
                + match t.size {
                    BitSize::Bits4 => x / 2,
                    _ => x,
                };
            let index = match t.size {
                BitSize::Bits4 => {
                    if x & 1 == 0 {
                        bytes[at] >> 4
                    } else {
                        bytes[at] & 0xf
                    }
                }
                _ => bytes[at],
            };
            img.put(y * width as usize + x, [index, 0, 0, 255]);
        }
    }
    let mirrored = texture::mirror_extend(
        &img,
        t.mirror_s,
        t.mirror_t,
        t.clamp_s,
        t.clamp_t,
        t.drawn_width as u32,
        t.drawn_height as u32,
    );
    Some(
        (0..(mirrored.width * mirrored.height) as usize)
            .map(|i| mirrored.get(i)[0])
            .collect(),
    )
}

/// Every `(model_file, graph_offset)` pair either of a fighter's two
/// `FTCommonPart` detail levels names (RE-242).
///
/// `ftDisplayMainProcDisplay` sets `G_LIGHTING` and `gDPSetRenderMode(
/// G_RM_FOG_PRIM_A, G_RM_AA_ZB_OPA_SURF2)` unconditionally before either
/// graph's own node lists run (RE-021, RE-244), so any
/// [`ssb_rom::mesh::convert_sequence`] call decoding one of these graphs
/// must seed [`ssb_rom::mesh::InitialMaterial::FIGHTER_EXTERNAL`] to match —
/// every other graph in the archive keeps the RDP reset default.
fn fighter_skeleton_graphs(loaded: &Loaded) -> std::collections::BTreeSet<(u32, u32)> {
    ssb_rom::fighter::FIGHTER_FILES
        .iter()
        .flat_map(|entry| {
            let main = loaded.files[entry.file as usize].as_ref();
            main.into_iter().flat_map(|main| {
                ssb_rom::fighter::common_parts(main, *entry)
                    .into_iter()
                    .flatten()
                    .map(|part| (part.model_file, part.graph))
            })
        })
        .collect()
}

/// Every stage's render-layer-1 `(file, graph_offset)` (RE-245).
///
/// `grDisplayLayer1PriProcDisplay`/`SecProcDisplay` set `G_ZBUFFER` and
/// `gDPSetRenderMode(G_RM_AA_ZB_OPA_SURF, G_RM_AA_ZB_OPA_SURF2)` unconditionally
/// before walking a stage's own layer-1 node lists -- see
/// [`ssb_rom::mesh::InitialMaterial::GROUND_LAYER1_EXTERNAL`] for the full
/// citation. Layers 0/2/3 need no entry here: their own external wrapper
/// clears `G_ZBUFFER` and sets a non-`ZB` render mode, which is already
/// [`ssb_rom::mesh::InitialMaterial::default`].
fn ground_layer1_graphs(loaded: &Loaded) -> std::collections::BTreeSet<(u32, u32)> {
    loaded
        .stages
        .iter()
        .flat_map(|stage| &stage.layers)
        .filter(|layer| layer.index == 1)
        .map(|layer| layer.graph)
        .collect()
}

/// Every loading-break transition scene's `(file, graph_offset)` (RE-246).
///
/// `ssb_rom::transition::ASSETS` already names each of the eleven
/// `dLBTransitionDescs` entries' exact file and graph offset (built for
/// `R0.13`'s framebuffer-capture work); `_(file, graph)` fields need no
/// further validation here, the same way [`ground_layer1_graphs`] trusts
/// `loaded.stages`.
fn lb_transition_graphs() -> std::collections::BTreeSet<(u32, u32)> {
    ssb_rom::transition::ASSETS
        .iter()
        .map(|asset| (asset.file, asset.graph))
        .collect()
}

/// The seed a [`ssb_rom::mesh::convert_sequence`] call for `(file,
/// graph_offset)` must use -- see [`fighter_skeleton_graphs`],
/// [`ground_layer1_graphs`] and [`lb_transition_graphs`].
fn initial_material_for(
    skeleton_graphs: &std::collections::BTreeSet<(u32, u32)>,
    ground_layer1_graphs: &std::collections::BTreeSet<(u32, u32)>,
    lb_transition_graphs: &std::collections::BTreeSet<(u32, u32)>,
    file: u32,
    graph_offset: u32,
) -> ssb_rom::mesh::InitialMaterial {
    if skeleton_graphs.contains(&(file, graph_offset)) {
        ssb_rom::mesh::InitialMaterial::FIGHTER_EXTERNAL
    } else if ground_layer1_graphs.contains(&(file, graph_offset)) {
        ssb_rom::mesh::InitialMaterial::GROUND_LAYER1_EXTERNAL
    } else if lb_transition_graphs.contains(&(file, graph_offset)) {
        ssb_rom::mesh::InitialMaterial::LB_TRANSITION_EXTERNAL
    } else {
        ssb_rom::mesh::InitialMaterial::default()
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
fn ground_layer1_list1_depth_seed(
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
fn convert_graph_at(
    loaded: &Loaded,
    file: &ssb_rom::archive::File,
    graph_offset: u32,
    plan: &[PlannedList],
    materials: &[ssb_rom::mobj::NodeMaterials],
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
    initial: ssb_rom::mesh::InitialMaterial,
) -> Vec<Result<ssb_rom::mesh::Mesh, ssb_rom::mesh::MeshError>> {
    use ssb_rom::mesh;

    let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
        .iter()
        .map(|p| {
            file.data
                .get(p.dl as usize..)
                .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                .unwrap_or_default()
        })
        .collect();
    let mat_anims = resolve_layer_mat_anims(loaded, file, graph_offset, materials, mat_anim_data);
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
    mesh::convert_sequence(&items, mesh::Source::of(file), initial)
}

/// Long enough that a looping script proves it loops (RE-089's own budget
/// for the same replay).
const MAT_ANIM_REPLAY_FRAMES: u32 = 600;

thread_local! {
    /// Primitives that named a sprite-table script before any primitive
    /// drawing its image did, and so carry no `mat_anim` (RE-326).
    static NON_OWNER_FIRST: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

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
fn resolve_one_mat_anim(
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
    let mut frames = 0u32;
    let mut seen_material = false;
    // RE-318: tile-0 UV tracks against their rest values, per replayed frame.
    let uv_rest: [f32; 4] = core::array::from_fn(|k| f32::from_bits(base_tracks[k + 1]));
    let mut uv_seen = [false; 4];
    let mut uv_replay_holds = true;
    let mut drives_lod = false;
    let mut max_current = 0.0f32;
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
        }
        if let Some(v) = j.track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_NEXT) {
            max_texture = max_texture.max(v);
        }
        seen_material |= (0..10).any(|track| j.track_value(track).is_some());
        drives_lod |= j.track_value(ssb_rom::matanim::TRACK_SET_LFRAC).is_some()
            || j.track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_NEXT)
                .is_some();
        for (k, seen) in uv_seen.iter_mut().enumerate() {
            if let Some(v) = j.track_value(ssb_rom::matanim::TRACK_TRA_U + k) {
                *seen = true;
                uv_replay_holds &= v == uv_rest[k];
            }
        }
        if j.ended() || j.looped() || frames >= MAT_ANIM_REPLAY_FRAMES {
            break;
        }
    }

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
        ssb_rom::mobj::read_sprites(file, sub_offset, texture_count as usize)?
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
        },
    ))
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
fn resolve_mat_anims(
    file: &ssb_rom::archive::File,
    matanim_table: u32,
    materials: &[ssb_rom::mobj::NodeMaterials],
    sub_at: impl Fn(usize, usize) -> Option<(u32, u16)>,
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
) -> Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> {
    let scripts = ssb_rom::matanim::resolve_scripts(file, matanim_table, materials.len(), |n| {
        materials[n].len()
    });
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
fn resolve_layer_mat_anims(
    loaded: &Loaded,
    file: &ssb_rom::archive::File,
    graph_offset: u32,
    materials: &[ssb_rom::mobj::NodeMaterials],
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
) -> Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> {
    let empty = || materials.iter().map(|c| vec![None; c.len()]).collect();

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

/// Builds the runtime asset pack: converted geometry and textures in the
/// layout the PSP consumes directly.
/// Report-only residual census (`docs/rendering/three-point-residuals.md`).
///
/// Rebuilds the pack with the residual recorder enabled, requires it to be
/// byte-identical to the reference pack, then measures every mesh texture
/// variant against the exact N64 3-point reference. Nothing is fixed.
fn residuals_cmd(path: &Path, opts: &[&str]) -> Res {
    let mut o = residuals::Options {
        json: "assets/generated/three-point-residuals.json".into(),
        markdown: "docs/rendering/three-point-residuals.md".into(),
        images: "assets/generated/three-point-residuals".into(),
        top_images: 20,
        refine: true,
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        pack_path: "assets/generated/three-point-residuals.pak".into(),
        pack_sha256: String::new(),
        rom_path: path.display().to_string(),
    };
    let mut reference = Some(PathBuf::from("assets/generated/ssb64.pak"));
    let mut only_file: Option<String> = None;
    let mut it = opts.iter();
    while let Some(opt) = it.next() {
        let mut value = || it.next().copied().ok_or(format!("{opt} needs a value"));
        match *opt {
            "--json" => o.json = value()?.into(),
            "--markdown" => o.markdown = value()?.into(),
            "--images" => o.images = value()?.into(),
            "--pack-out" => o.pack_path = value()?.into(),
            "--top-images" => o.top_images = value()?.parse()?,
            "--threads" => o.threads = value()?.parse()?,
            "--reference-pack" => {
                let v = value()?;
                reference = (v != "none").then(|| v.into());
            }
            "--no-refine" => o.refine = false,
            // Development only: one file, no reference-pack check.
            "--file" => {
                only_file = Some(value()?.to_string());
                reference = None;
            }
            other => return Err(format!("unknown option {other}").into()),
        }
    }
    residuals::enable();
    let pack_out = o.pack_path.display().to_string();
    let mut pack_opts = vec!["--out", pack_out.as_str()];
    if let Some(f) = only_file.as_deref() {
        pack_opts.extend(["--file", f]);
    }
    if let Err(e) = pack(path, &pack_opts) {
        // A single-file pack cannot resolve cross-file objects; its mesh
        // textures are already recorded by then.
        if only_file.is_none() {
            return Err(e);
        }
        eprintln!("residuals: single-file pack stopped early: {e}");
    }
    let built = fs::read(&o.pack_path).unwrap_or_default();
    if let Some(reference) = reference {
        let expected = fs::read(&reference).map_err(|e| format!("{}: {e}", reference.display()))?;
        if expected != built {
            return Err(format!(
                "recorded pack differs from {}: the report would not describe the shipped pack",
                reference.display()
            )
            .into());
        }
    }
    o.pack_sha256 = residuals::sha256_hex(&built);
    residuals::run(&o)
}

fn pack(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::{mesh, pack as fmt};

    let mut out_path = PathBuf::from("assets/generated/ssb64.pak");
    let mut only_file: Option<u32> = None;
    let mut swizzle = true;

    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--out" => out_path = it.next().ok_or("--out needs a path")?.into(),
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            // For bisecting on-device texture bugs: swizzling is a pure
            // reordering, so if output changes with it off, the swizzler or the
            // GE's swizzle flag is at fault rather than the decode.
            "--no-swizzle" => swizzle = false,
            // Report-only A/B packs: also apply the named section 11
            // review-candidate overrides (`v397,v1004` or `all`). The
            // shipped pack is built without this option.
            "--residual-candidates" => {
                let list = it.next().ok_or("--residual-candidates needs a list")?;
                let variants: Vec<u16> = if *list == "all" {
                    residual_fixes::REVIEW_CASES
                        .iter()
                        .map(|c| c.variant)
                        .collect()
                } else {
                    list.split(',')
                        .map(|v| v.trim_start_matches('v').parse::<u16>())
                        .collect::<Result<_, _>>()?
                };
                residual_fixes::enable_candidates(variants)?;
            }
            // Report-only: zero the alpha of every packed direct override
            // (cutout sites) to capture the frame without them.
            "--residual-hide-probe" => residual_fixes::enable_hide_probe(),
            // Report-only (RE-318): draw nothing for primitives the wide-tile
            // lowering produced, so a capture shows the frame without them.
            "--hide-wide-tiles" => {
                let _ = HIDE_WIDE_TILES.set(true);
            }
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let mut writer = fmt::PackWriter::new();
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
    // A stage names its render layers by the `DObjDesc` address they start at,
    // and `add_object` is given that same address -- so the layer lookup is an
    // exact match, never a search.
    let mut object_index: BTreeMap<(u32, u32), u32> = BTreeMap::new();

    let loaded = load_all(&archive);
    let skeleton_graphs = fighter_skeleton_graphs(&loaded);
    let ground_graphs = ground_layer1_graphs(&loaded);
    let transition_graphs = lb_transition_graphs();

    for id in 0..archive.len() as u32 {
        if only_file.is_some_and(|f| f != id) {
            continue;
        }
        let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
            continue;
        };

        let all = ssb_rom::scan::find_root_display_lists(file);
        // convert() inlines G_DL callees, so packing a list that another list
        // calls would duplicate its geometry.
        let called: std::collections::BTreeSet<u32> =
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
            .map(|g| plan_draw_order(g, &resolver))
            .collect();
        let authoritative: std::collections::BTreeSet<u32> = plans
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
            let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                .iter()
                .map(|p| {
                    file.data
                        .get(p.dl as usize..)
                        .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                        .unwrap_or_default()
                })
                .collect();
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
                let Ok(m) = converted else { continue };
                if m.triangle_count() == 0 {
                    continue;
                }
                let index = pack_mesh(
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
            let Ok(m) = mesh::convert(&dl.commands, mesh::Source::of(file)) else {
                continue;
            };
            if m.triangle_count() == 0 {
                continue;
            }
            pack_mesh(
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
        if id == 297 {
            const FIREBALL_MOBJ_TABLE: u32 = 0xD8;
            const FIREBALL_DISPLAY_LIST: u32 = 0x1D8;
            if let (Some(materials), Ok(cmds)) = (
                ssb_rom::mobj::read_table(file, FIREBALL_MOBJ_TABLE, 1),
                ssb_rom::dl::decode_list_at(
                    &file.data[FIREBALL_DISPLAY_LIST as usize..],
                    FIREBALL_DISPLAY_LIST,
                ),
            ) {
                let item = mesh::SequenceItem {
                    cmds: &cmds,
                    world: ssb_rom::scene::Mat4::IDENTITY,
                    mobjs: &materials.nodes[0],
                    mat_anims: &[],
                    depth_seed: None,
                    stream: 0,
                };
                if let Some(Ok(fireball)) = mesh::convert_sequence(
                    &[item],
                    mesh::Source::of(file),
                    mesh::InitialMaterial::WEAPON_EXTERNAL,
                )
                .into_iter()
                .next()
                {
                    if fireball.triangle_count() != 0 {
                        pack_mesh(
                            &mut writer,
                            &mut tex_index,
                            &mut mat_anim_index,
                            &mat_anim_data,
                            Texels {
                                home: file,
                                all: &loaded.files,
                            },
                            id,
                            FIREBALL_DISPLAY_LIST,
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
                            stream: 0,
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

        for (gi, graph) in graphs.iter().enumerate() {
            node_dls += graph.display_lists().count();
            placed_meshes += node_mesh[gi].iter().filter(|m| m.is_some()).count();
            let object = writer.add_object(graph, id, |n| node_mesh[gi][n], &node_extra[gi]);
            object_index.insert((id, graph.offset), object);
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
            let costumes = fighter_costume_count(id);
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
                    &loaded,
                    file,
                    graph.offset,
                    plan,
                    &base_materials,
                    &mut mat_anim_data,
                    initial,
                );
                for costume in 1..costumes {
                    let materials_k = loaded.materials_at(file, graph, costume as f32);
                    let converted_k = convert_graph_at(
                        &loaded,
                        file,
                        graph.offset,
                        plan,
                        &materials_k,
                        &mut mat_anim_data,
                        initial,
                    );
                    for (p, m_k) in plan.iter().zip(&converted_k) {
                        if !p.own_space() {
                            continue;
                        }
                        let Ok(m_k) = m_k else { continue };
                        if m_k.triangle_count() == 0 {
                            continue;
                        }
                        if base_converted.get(p.node) == Some(&Ok(m_k.clone())) {
                            continue;
                        }
                        let variant_index = pack_mesh(
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
            let file = loaded.files.get(f as usize)?.as_ref()?;
            ssb_rom::collision::read(file, at)
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
            let Some(file) = loaded.files.get(af as usize).and_then(Option::as_ref) else {
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
    for (kind, entry) in ssb_rom::anim::FIGHTER_ANIMS.iter().enumerate() {
        let file_entry = ssb_rom::fighter::FIGHTER_FILES[kind];
        let nodes = loaded.files[file_entry.file as usize]
            .as_ref()
            .and_then(|main| {
                let part = ssb_rom::fighter::common_parts(main, file_entry)[0]?;
                let mask = ssb_rom::fighter::setup_parts(main, file_entry)?;
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
            let frames = ssb_rom::anim::decode_length(id as u32, &file)
                .ok()
                .and_then(|l| l.frames())
                .unwrap_or(0);
            // The motion descriptor, not the table length alone, identifies
            // a leading runtime joint (`TransN`, `XRotN`, or `YRotN`). A
            // disabled trailing model part can also leave one more script
            // than active joints; treating that as a leading joint shifts
            // every model transform and sinks the visible fighter (RE-331).
            let hidden_joint =
                table.len() == nodes.len() + 1 && ssb_rom::anim::LEADING_RUNTIME_JOINT[kind][slot];
            let joints: Vec<(Option<u32>, Option<u32>)> = table
                .iter()
                .enumerate()
                .map(|(j, &at)| {
                    let node = if hidden_joint {
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
    }

    // Now that the fighter block is complete and dense, the stage entries can
    // go after it without disturbing `fighter_anim`'s indexing.
    for (stage_index, af, joints) in deferred_stage_anims {
        let Some(file) = loaded.files.get(af as usize).and_then(Option::as_ref) else {
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
            .and_then(Option::as_ref)
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
        .zip(ssb_rom::effect::MANAGER_EFFECT_ANIM_JOINTS)
        .enumerate()
    {
        let Some(anim_at) = anim_at else { continue };
        let file = loaded
            .files
            .get(file_id as usize)
            .and_then(Option::as_ref)
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

    // Independent LBParticle banks live outside relocData. Decode and convert
    // them after scene work so their frame textures append without disturbing
    // any existing material texture indices (RE-180/181).
    let mut particle_scripts = 0usize;
    let mut particle_textures = 0usize;
    let mut particle_frames = 0usize;
    for &spec in ssb_rom::particle::BANKS {
        let (scripts, textures) = ssb_rom::particle::decode_bank(&data, spec)
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

    // `ftShadowProcDisplay` is not a DObj/display-list mesh: it loads this
    // I4 image directly and writes its own four-to-eight vertices every
    // frame.  Extract its first 16x16 frame explicitly rather than hoping an
    // effect bank happens to retain the same bytes.  The N64 uses mirror+wrap
    // on both axes; bake one mirrored 32x32 period so the PSP can repeat it
    // with ordinary `sceGuTexWrap(Repeat)` without changing the source UVs.
    let shadow_file = loaded
        .files
        .get(84)
        .and_then(Option::as_ref)
        .ok_or("fighter shadow source file 84 missing")?;
    let shadow = fighter_shadow_texture(&shadow_file.data, swizzle)?;
    writer.add_fighter_shadow_texture(&shadow);

    let bytes = writer.finish();
    if let Some(dir) = out_path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&out_path, &bytes)?;

    // Verify what we just wrote actually loads, rather than trusting it.
    let pack = ssb_rom::pack::Pack::open(&bytes)
        .map_err(|e| format!("wrote a pack that will not load: {e:?}"))?;

    println!("asset pack -> {}", out_path.display());
    println!("  meshes      {meshes}");
    println!("  triangles   {triangles}");
    // One GE draw call per primitive, so this is the number the state-sorting
    // in `merge_by_material` exists to hold down.
    println!(
        "  draws       {} ({:.1} triangles each)",
        pack.prim_count(),
        triangles as f64 / pack.prim_count().max(1) as f64
    );
    println!("  textures    {}", pack.texture_count());
    println!(
        "  objects     {objects} ({} nodes, {placed_meshes}/{node_dls} node lists placed)",
        pack.node_count()
    );
    if extra_leaves > 0 {
        println!(
            "  extra       {extra_leaves} leaf nodes for lists a node could not hold \
             (extra link entries, and pre-matrix pair halves drawn in the parent's space)"
        );
    }
    if costume_overrides_added > 0 {
        println!(
            "  costumes    {costume_overrides_added} per-(node, costume) mesh substitutions \
             ({} pack entries, RE-098)",
            pack.costume_override_count()
        );
    }
    println!(
        "  stages      {} ({with_collision} with collision, \
         {resolved_layers}/{stage_layers} render layers resolved)",
        pack.stage_count()
    );
    println!(
        "  collision   {} lines, {} vertices, {} map points",
        pack.line_count(),
        pack.coll_vertex_count(),
        pack.point_count()
    );
    println!(
        "  fighters    {fighters}/{}",
        ssb_rom::fighter::FIGHTER_FILES.len()
    );
    if !fighters_failed.is_empty() {
        println!(
            "              did not decode: {}",
            fighters_failed.join(", ")
        );
    }
    println!(
        "  animations  {packed_anims} ({anim_joints_packed} joint entries, {} joints bound to a node, {hidden_joint_anims} with a runtime joint)",
        pack_anim_joints_bound(&pack)
    );
    if !anims_failed.is_empty() {
        println!("              did not pack: {}", anims_failed.join(", "));
    }
    // Read back from the *written* pack, not the writer's own vectors: the
    // flag has to survive serialisation to be worth anything on device.
    let billboards = (0..pack.node_count())
        .filter_map(|i| pack.node(i))
        .filter(|n| n.flags & fmt::NodeDesc::FLAG_BILLBOARD != 0)
        .count();
    let mipped = (0..pack.texture_count())
        .filter_map(|i| pack.texture(i))
        .filter(|t| t.levels > 1)
        .count();
    println!(
        "  mipmaps     {mipped} of {} texture(s) carry extra levels",
        pack.texture_count()
    );
    let (wide_prims, wide_split, wide_emitted) = WIDE_TILE_STATS.with(|s| *s.borrow());
    println!(
        "  wide tiles  {wide_prims} primitive(s) over the GE's 512-texel limit lowered to {wide_emitted} (RE-318, {wide_split} triangle(s) split)"
    );
    LOD_BLEND_SITES.with(|s| {
        let sites = s.borrow();
        let exact = sites.iter().filter(|site| site.outcome.is_ok()).count();
        println!(
            "  lod blends  {exact} of {} classified two-tile blend primitive(s) drawn in two passes (RE-321)",
            sites.len()
        );
        for site in sites.iter() {
            match site.outcome {
                Ok(next) => println!(
                    "    file {} list 0x{:X} prim {}: {} triangle(s), {next} TEXEL1 image(s)",
                    site.file, site.dl, site.prim, site.triangles
                ),
                Err(why) => println!(
                    "    file {} list 0x{:X} prim {}: {} triangle(s) declined: {why}",
                    site.file, site.dl, site.prim, site.triangles
                ),
            }
        }
    });
    println!("  billboards  {billboards} node(s) drawn facing the camera");
    println!("  stage anims {stage_anims} stage(s), {stage_anim_joints} animated node(s)");
    println!(
        "  transitions {} wipe(s), {transition_anim_joints} animated node(s)",
        ssb_rom::transition::ASSETS.len()
    );
    println!("  effect anims {effect_anims} effect(s), {effect_anim_joints} animated node(s)");
    println!(
        "  particles   {} bank(s), {particle_scripts} script(s), {particle_textures} texture series, {particle_frames} frame(s)",
        pack.particle_bank_count()
    );
    let mat_animated_textures = (0..pack.texture_count())
        .filter_map(|i| pack.texture(i))
        .filter(|t| t.mat_anim != fmt::TextureDesc::NO_ANIM)
        .count();
    println!(
        "  mat anims   {} script(s), {} palette variant(s), {mat_animated_textures} texture(s) animated (RE-089/090/091)",
        pack.mat_anim_count(),
        pack.mat_anim_palette_count()
    );
    println!("  size        {:.1} KiB", bytes.len() as f64 / 1024.0);
    println!("  verified    loads back cleanly");

    // Rank objects by the geometry they actually place. This is the number the
    // on-device viewer opens on, and it is the honest measure of how much of
    // the scene graph is doing useful work.
    let mut ranked: Vec<(u32, u32, u32)> = Vec::new();
    for i in 0..pack.object_count() {
        let Some(o) = pack.object(i) else { continue };
        let mut tris = 0u32;
        for k in 0..o.node_count {
            let Some(n) = pack.node(o.first_node + k) else {
                continue;
            };
            let Some(m) = (n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                .then(|| pack.mesh(n.mesh))
                .flatten()
            else {
                continue;
            };
            for j in 0..m.prim_count {
                if let Some(pr) = pack.prim(m.first_prim + j) {
                    tris += pr.index_count / 3;
                }
            }
        }
        ranked.push((i, tris, o.source_file));
    }
    ranked.sort_by_key(|&(_, t, _)| core::cmp::Reverse(t));
    let placed_tris: u32 = ranked.iter().map(|&(_, t, _)| t).sum();
    println!(
        "  in objects  {placed_tris} triangles ({:.0}% of the pack)",
        placed_tris as f64 / triangles.max(1) as f64 * 100.0
    );
    println!("  top objects:");
    for (i, t, f) in ranked.iter().take(5) {
        println!("    object {i:<4} file {f:<5} {t} triangles");
    }
    println!(
        "  {} primitive(s) named a sprite-table script before its image owner",
        NON_OWNER_FIRST.with(|n| n.get())
    );
    Ok(())
}

/// Extracts the 16x16 I4 frame `ftShadowProcDisplay` loads from
/// `dEFCommonEffects2_Shadow_TextureImage` (file 84, `0x3A68`).  The trailing
/// 5,152 bytes are other frames in the shared effect pool; the fighter path
/// names the base address and `gDPLoadTextureBlock_4b` reads only 128 bytes.
fn fighter_shadow_texture(
    file: &[u8],
    swizzle: bool,
) -> Result<ssb_rom::psp_texture::PspTexture, Box<dyn std::error::Error>> {
    use ssb_rom::psp_texture::{self, Psm};
    use ssb_rom::texture::{self, Rgba8};

    const OFFSET: usize = 0x3A68;
    const WIDTH: u32 = 16;
    const HEIGHT: u32 = 16;
    let bytes = file
        .get(OFFSET..OFFSET + (WIDTH * HEIGHT / 2) as usize)
        .ok_or("fighter shadow image is truncated")?;
    let mut image = Rgba8::new(WIDTH, HEIGHT);
    for (byte_index, &byte) in bytes.iter().enumerate() {
        for (nibble_index, nibble) in [byte >> 4, byte & 0x0F].into_iter().enumerate() {
            let intensity = nibble * 17;
            image.put(
                byte_index * 2 + nibble_index,
                [intensity, intensity, intensity, intensity],
            );
        }
    }
    let mirrored = texture::mirror_extend(&image, true, true, false, false, WIDTH, HEIGHT);
    Ok(psp_texture::pack_rgba(&mirrored, Psm::Psm8888, swizzle))
}

fn convert_particle_frame(
    texture: &ssb_rom::particle::Texture<'_>,
    frame: usize,
    swizzle: bool,
) -> Result<ssb_rom::psp_texture::PspTexture, String> {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture::{self, Format};

    let image = texture
        .images
        .get(frame)
        .ok_or_else(|| "frame out of range".to_string())?;
    let palette_index = if texture.flags & 1 != 0 { 0 } else { frame };
    let palette: Vec<u16> = texture
        .palettes
        .get(palette_index)
        .map(|bytes| {
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|word| u16::from_be_bytes(*word))
                .collect()
        })
        .unwrap_or_default();

    if texture.format == Format::Ci {
        return psp::pack_paletted(
            image,
            texture.width,
            texture.height,
            texture.size,
            &palette,
            swizzle,
        )
        .map_err(|error| format!("{error:?}"));
    }
    if texture.format == Format::I {
        return psp::pack_indexed(
            image,
            texture.width,
            texture.height,
            texture.size,
            &psp::intensity_palette(texture.size),
            swizzle,
        )
        .map_err(|error| format!("{error:?}"));
    }
    let rgba = texture::decode(
        image,
        texture.width,
        texture.height,
        texture.format,
        texture.size,
        None,
    )
    .map_err(|error| format!("{error:?}"))?;
    Ok(psp::pack_rgba(
        &rgba,
        psp::choose_psm(texture.format, texture.size),
        swizzle,
    ))
}

/// Resolves `G_SETTILE.palette`'s raw bank index (RE-223/`R2.0`/P2) into a
/// byte offset/entry-count to read a loaded TLUT from: bank `n` starts
/// `n * 16` entries into the chunk `G_LOADTLUT` loaded. A bank beyond the
/// loaded chunk should not occur on real content (RE-223's own archive-wide
/// measurement), but is guarded here rather than panicking or silently
/// returning an empty palette: falls back to bank 0 (`palette_offset`/
/// `palette_entries` unshifted). A `palette` of 0 (the overwhelming common
/// case) is always a no-op by construction.
fn palette_bank_offset(palette_offset: u32, palette_entries: u16, palette: u8) -> (u32, usize) {
    let bank = palette as usize * 16;
    let entries = palette_entries.max(1) as usize;
    if bank < entries {
        (palette_offset + (bank * 2) as u32, entries - bank)
    } else {
        (palette_offset, entries)
    }
}

/// Decodes and packs one texture referenced by a primitive.
///
/// The texels and the palette are looked up independently, because they need
/// not be in the same file: a fighter's palette comes from its own file while
/// a stage's texels come from a shared one.
#[allow(clippy::too_many_arguments)]
fn convert_texture(
    src: Texels<'_>,
    t: &ssb_rom::mesh::TextureRef,
    coverage: &filter_coverage::Coverage,
    swizzle: bool,
    animated_palettes: Option<&[Vec<u32>]>,
    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy,
    mut compensated_out: Option<&mut bool>,
    direct_fit: Option<residuals::CrossFit>,
) -> Option<ssb_rom::psp_texture::PspTexture> {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture;

    let file = src.bytes(t.data_file)?;
    if (t.data_offset >> 24) != 0 || (t.data_offset == 0 && t.data_file.is_none()) {
        return None; // segmented, or a pointer nothing resolved
    }
    // RE-283: file 324's boot texture (Link's, offset 0xCF18, 16x16 CI4)
    // reproduces a genuine PSP GE rendering defect (a purple/black patch) on
    // both PPSSPP and physical hardware, given a proven byte-correct
    // texture/CLUT bind -- the same class of defect RE-267 already found and
    // fixed for Donkey Kong's file 317 by bypassing PSP's paletted-texture
    // transport entirely. Widening the row (RE-283's own stride experiment,
    // rejected: the defect persisted byte-identical even once this texture
    // was widened enough to actually swizzle) ruled out stride/swizzle as the
    // mechanism, matching RE-267's own "swizzling... did not explain" finding.
    let is_link_boot = src.home.id == 324 && t.data_file.is_none() && t.data_offset == 0xCF18;
    // RE-283 (fifteenth follow-up): file 324's rear-leg shin-cuff texture
    // (Link's, offset 0xB4E0, 16x32 CI4, same 8-byte-row shape as the boot
    // texture above) node-isolated to node 23 only -- its mirrored twin
    // (node 28, same texture, `G_SETTILE.cms` mirror instead of wrap)
    // renders clean. Repeat-wrapped sampling of this exact packed shape
    // reproduces the same purple/black speckle class as the boot defect;
    // the same paletted-transport bypass that fixed it applies here too.
    let is_link_shin = src.home.id == 324 && t.data_file.is_none() && t.data_offset == 0xB4E0;
    // RE-283 (Yoshi follow-up): file 338's head node (node 3) binds two small
    // CI4 textures under `Clamp`/`Clamp` addressing (no mirror) -- the mouth-
    // corner mark (offset 0x9518, 8x8) and both eyes (offset 0x9BD0, 16x16,
    // bound twice). Node-isolating (`RE283_ONLY_LOCAL_NODE=3`) reproduced the
    // user's "heavy scrambled multicolour noise" and "smaller patch near the
    // nostril" exactly; disabling each texture in turn localized the ridge
    // noise to the eye texture and the mouth-corner mark to this one, and
    // disabling both together left the head perfectly clean. Both are the
    // same small-paletted packed shape (8-byte-row-or-narrower `PsmT4`) as
    // Link's boot/shin-cuff defect, this time reached through `Clamp`
    // addressing rather than `Wrap` -- the same GE quirk is not gated on
    // wrap mode specifically, just this texture-size class. Same fix.
    let is_yoshi_head = src.home.id == 338
        && t.data_file.is_none()
        && (t.data_offset == 0x9518 || t.data_offset == 0x9BD0);
    // RE-283 (Captain Falcon follow-up): file 332's shoulder-pad node (node
    // 10, graph 0x3BE0) binds an 8x8 CI4 dither texture at offset 0xC508 --
    // a prior session misidentified this node's content via a desynced raw
    // `dlraw` command dump (offsets 0xC128/0xBBB0, neither real) and ruled
    // out this same bypass class against those wrong textures. Node-isolating
    // the *correct* offset (recovered from the authoritative pack()/
    // plan_draw_order pipeline) reproduces the user's "heavily scrambled
    // yellow/orange/black patch" exactly, and this bypass alone clears it
    // completely -- same small-paletted-texture (8-byte-row-or-narrower
    // PsmT4) GE quirk as Link's boot/shin-cuff and Yoshi's head.
    let is_falcon_shoulder = src.home.id == 332 && t.data_file.is_none() && t.data_offset == 0xC508;
    // RE-283 (Ness follow-up): file 335's node 4 (right shoulder/sleeve, graph
    // 0x26B0) binds an 8x8 CI4 texture at offset 0xBDE8, mirror-narrowed to
    // 8x16 -- the same small-paletted packed shape (8-byte-row-or-narrower
    // `PsmT4`) as Link's boot/shin-cuff, Yoshi's head and Falcon's shoulder.
    // A raw `dlraw` decode of this node's list first desynced onto the wrong
    // offsets entirely (0xAD00/0xAE30/0xAB20, all clean 16x32 shirt-stripe
    // textures unaffected by bypassing them); the authoritative
    // `pack()`/`plan_draw_order` pipeline (`romtool scene --dump-node 4`)
    // recovered the real bound texture. Node-isolating (`RE283_ONLY_LOCAL_NODE=4`)
    // reproduced the user's "right shoulder strap area shows scrambled pixel
    // noise" exactly; this bypass alone clears it completely.
    let is_ness_shoulder = src.home.id == 335 && t.data_file.is_none() && t.data_offset == 0xBDE8;
    // The natural PSP format. A 4- or 8-bit texel drawn with the TLUT on is a
    // palette index whatever its N64 format says (RE-313), so it stays
    // indexed; everything else follows `choose_psm`.
    let natural = match (t.tlut.enabled(), t.size) {
        (true, texture::BitSize::Bits4) => psp::Psm::PsmT4,
        (true, texture::BitSize::Bits8) => psp::Psm::PsmT8,
        _ => psp::choose_psm(t.format, t.size),
    };
    // The RE-283 bypass replaces only the paletted *transport*. The texels
    // still enter the same compensation pipeline below as RGBA8888.
    let psm = if (src.home.id == 317
        || is_link_boot
        || is_link_shin
        || is_yoshi_head
        || is_falcon_shoulder
        || is_ness_shoulder)
        && natural.is_paletted()
    {
        psp::Psm::Psm8888
    } else {
        natural
    };

    // A CI4 bank is a 16-entry window of the loaded TLUT; an 8-bit index
    // ignores the bank (`fetch_texel_entlut_quadro`), and RE-223 found a
    // nonzero bank only on CI4.
    let bank = if t.size == texture::BitSize::Bits4 {
        t.palette
    } else {
        0
    };
    let mut tlut: Vec<u16> = match t.palette_offset {
        Some(off) => {
            let (off, n) = palette_bank_offset(off, t.palette_entries, bank);
            src.bytes(t.palette_file)
                .and_then(|f| f.get(off as usize..off as usize + n * 2))
                .map(texture::parse_tlut)
                .unwrap_or_default()
        }
        None => Vec::new(),
    };
    if t.tlut.enabled() && tlut.is_empty() {
        // No TLUT the converter can resolve: nothing defined to sample.
        return None;
    }
    if t.tlut.enabled() {
        extend_short_tlut(src, file, t, &mut tlut);
    }
    // The colour each index value yields, for an indexed natural format.
    let index_colours: Vec<u32> = if t.tlut.enabled() {
        tlut.iter()
            .map(|&e| psp::pack_abgr(t.tlut.entry_rgba(e)))
            .collect()
    } else if natural.is_paletted() && t.format == texture::Format::Ci {
        // CI with the TLUT off reads the raw index as intensity (RE-313).
        let n = if t.size == texture::BitSize::Bits4 {
            16
        } else {
            256
        };
        (0..n)
            .map(|i| {
                let v = if n == 16 {
                    (t.palette << 4) | i as u8
                } else {
                    i as u8
                };
                psp::pack_abgr([v, v, v, v])
            })
            .collect()
    } else if natural.is_paletted() && t.format == texture::Format::I {
        // An intensity texture carries no palette because on the N64 it needs
        // none. Generating the ramp keeps it at 4 or 8 bits instead of letting
        // it fall through to the 8888 path `choose_psm` picked `PsmT4` to
        // avoid (RE-047).
        psp::intensity_palette(t.size)
    } else {
        Vec::new()
    };
    if natural.is_paletted() && index_colours.is_empty() {
        return None;
    }

    // Mipmapped, because the N64's textures are frequently dithered and a
    // dithered gradient sampled near one texel per pixel aliases into moire
    // instead of reading as shading (RE-053). Every level is generated from the
    // decoded image; for a paletted texture level 0 comes back bit-identical,
    // since each texel's nearest palette entry is the one it was decoded from.
    // `t.width`/`t.height` are already narrowed to one repeat period
    // (`mesh.rs`'s `current_texture()`, RE-044), so a mirrored axis here
    // pre-bakes exactly the real hardware's mirror-repeat into the pixel
    // data -- a plain `Repeat` wrap over the resulting wider/taller image
    // reproduces it exactly, since `sceGuTexScale` renormalises UVs against
    // whatever dimensions the packed texture actually reports (RE-067).
    let decode_mirrored = |tlut: &[u16]| {
        let img = decode_texture(file, t, tlut)?;
        Some(texture::mirror_extend(
            &img,
            t.mirror_s,
            t.mirror_t,
            t.clamp_s,
            t.clamp_t,
            t.drawn_width as u32,
            t.drawn_height as u32,
        ))
    };

    let mut mipped = |palette: Vec<u32>| {
        let img = decode_mirrored(&tlut)?;
        // RE-311: a bounded texgen primitive trains on its real-normal pose
        // coverage plus a one-point-per-texel regularization over the whole
        // reachable box, and must pass two holdouts: the disjoint-pose
        // real-normal set and the conservative old-density lattice over that
        // box (every point of which some pose can generate).
        let texgen_meta = coverage.texgen.as_ref();
        let active_texgen = texgen_meta.filter(|meta| !meta.legacy);
        let merged_train: Vec<[i32; 2]>;
        let train_samples: &[[i32; 2]] = match active_texgen {
            Some(meta) => {
                let mut merged = coverage.train.clone();
                merged.extend(meta.bound.regularization());
                merged.sort_unstable();
                merged.dedup();
                merged_train = merged;
                &merged_train
            }
            None => &coverage.train,
        };
        let conservative: Vec<[i32; 2]> = texgen_meta
            .map(|meta| meta.bound.lattice(4))
            .unwrap_or_default();
        let conservative_gate = |candidate: &ssb_rom::texture::Rgba8| {
            active_texgen.is_none() || {
                let before = ssb_rom::filter_compensation::measure_samples_policy(
                    &img,
                    &img,
                    t.clamp_s,
                    t.clamp_t,
                    &conservative,
                    alpha_policy,
                );
                let after = ssb_rom::filter_compensation::measure_samples_policy(
                    &img,
                    candidate,
                    t.clamp_s,
                    t.clamp_t,
                    &conservative,
                    alpha_policy,
                );
                validation_preserved(before, after, alpha_policy)
            }
        };
        let texgen_report = |candidate: &ssb_rom::texture::Rgba8,
                             accepted: bool,
                             outcome: &str,
                             solve_sse: Option<(u64, u64)>| {
            let Some(meta) = texgen_meta else { return };
            let full_tile: Vec<[i32; 2]> = (4..img.height as i32 * 32)
                .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                .flat_map(|v| {
                    (4..img.width as i32 * 32)
                        .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                        .map(move |u| [u, v])
                })
                .collect();
            let mut fields = String::new();
            for (name, set) in [
                ("real", meta.holdout.as_slice()),
                ("box", conservative.as_slice()),
                ("tile", full_tile.as_slice()),
            ] {
                let a = ssb_rom::filter_compensation::measure_samples_policy(
                    &img,
                    &img,
                    t.clamp_s,
                    t.clamp_t,
                    set,
                    alpha_policy,
                );
                let b = ssb_rom::filter_compensation::measure_samples_policy(
                    &img,
                    candidate,
                    t.clamp_s,
                    t.clamp_t,
                    set,
                    alpha_policy,
                );
                fields.push_str(&format!(
                    " {name}_samples={} {name}_source_sse={} {name}_final_sse={} {name}_source_above32={} {name}_final_above32={} {name}_source_max={} {name}_final_max={}",
                    a.samples, a.squared_error, b.squared_error, a.above_32, b.above_32, a.max, b.max
                ));
            }
            eprintln!(
                "texgen-filter-comp file={} offset={:#X} {}x{} mode={} linear={} box={:?}..{:?} train={} regularization={} accepted={} outcome={} solve_baseline_sse={} solve_compensated_sse={}{}",
                t.data_file.map_or(src.home.id, u32::from),
                t.data_offset,
                img.width,
                img.height,
                if meta.legacy { "full-tile" } else { "real" },
                meta.linear,
                meta.bound.min,
                meta.bound.max,
                coverage.train.len(),
                if meta.legacy { 0 } else { meta.bound.regularization().len() },
                accepted,
                outcome,
                solve_sse.map_or(-1, |s| s.0 as i64),
                solve_sse.map_or(-1, |s| s.1 as i64),
                fields
            );
        };
        // A shifted full-tile grid is the holdout for unbounded texgen (the
        // fallback). Authored UVs use the separate cell-local holdout built
        // from their actual triangles.
        let validation_dense: Vec<[i32; 2]> = if train_samples.is_empty() {
            (4..img.height as i32 * 32)
                .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                .flat_map(|v| {
                    (4..img.width as i32 * 32)
                        .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                        .map(move |u| [u, v])
                })
                .collect()
        } else {
            Vec::new()
        };
        let validation_samples = if !coverage.validation.is_empty() {
            coverage.validation.as_slice()
        } else if !validation_dense.is_empty() {
            validation_dense.as_slice()
        } else {
            &[]
        };
        // Report-only (`romtool residuals`): clones what this conversion
        // packs; it never changes the result.
        let record = |final_img: &ssb_rom::texture::Rgba8,
                      final_psm: psp::Psm,
                      method: &'static str,
                      attempted: bool,
                      animated: Option<residuals::Animated>| {
            if residuals::enabled() {
                residuals::record_conversion(residuals::Conversion {
                    texture: *t,
                    home: src.home.id,
                    source: img.clone(),
                    final_level0: final_img.clone(),
                    source_psm: natural,
                    final_psm,
                    palette: index_colours.clone(),
                    animated,
                    method,
                    attempted,
                    policy: alpha_policy,
                });
            }
        };
        // RE-312 exact-use-site override (`residual_fixes::Kind::Direct`):
        // the section 11 cross-phase fit on this site's own phase-uniform
        // set, shipped as an immutable RGBA8888 variant. Only listed
        // authored-UV, single-palette sites reach here.
        if let Some(mode) = direct_fit {
            assert!(
                animated_palettes.is_none() && coverage.texgen.is_none(),
                "direct override needs a static authored-UV site"
            );
            assert!(
                !coverage.validation.is_empty(),
                "direct override needs a holdout"
            );
            let mut eval = coverage.validation.clone();
            eval.sort_unstable_by_key(|p| (p[1], p[0]));
            eval.dedup();
            let u1 = residuals::phase_uniform_set(&eval);
            let mut fitted =
                residuals::cross_phase_fit(&img, [t.clamp_s, t.clamp_t], alpha_policy, &u1, mode);
            if residual_fixes::hide_probe() {
                assert!(matches!(
                    alpha_policy,
                    ssb_rom::filter_compensation::AlphaPolicy::Cutout { .. }
                ));
                fitted
                    .pixels
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .for_each(|p| p[3] = 0);
            }
            record(
                &fitted,
                psp::Psm::Psm8888,
                "re312-direct-override",
                true,
                None,
            );
            if let Some(out) = compensated_out.as_deref_mut() {
                *out = true;
            }
            return Some(psp::pack_mipped(&fitted, psp::Psm::Psm8888, &[], swizzle));
        }
        if psm.is_paletted() && t.format == texture::Format::Ci {
            if let Some(states) = animated_palettes.filter(|states| !states.is_empty()) {
                let source = decode_index_field(file, t)?;
                let limit = if psm == psp::Psm::PsmT4 {
                    16
                } else {
                    palette.len()
                };
                if states.iter().all(|p| p.len() >= limit) {
                    let dense: Vec<[i32; 2]>;
                    let samples = if train_samples.is_empty() {
                        dense = (0..img.height as i32 * 32)
                            .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                            .flat_map(|v| {
                                (0..img.width as i32 * 32)
                                    .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                                    .map(move |u| [u, v])
                            })
                            .collect();
                        dense.as_slice()
                    } else {
                        train_samples
                    };
                    let optimized = ssb_rom::filter_compensation::optimize_animated_indices(
                        &source, img.width, img.height, states, limit, t.clamp_s, t.clamp_t,
                        samples,
                    );
                    let mut before = 0u64;
                    let mut after = 0u64;
                    let mut better_large = false;
                    let mut nonregressing = true;
                    let mut baseline_large = 0u64;
                    let mut baseline_samples = 0u64;
                    let mut baseline_max = 0u8;
                    let mut validation_before = 0u64;
                    let mut validation_after = 0u64;
                    let mut validation_before_32 = 0u64;
                    let mut validation_after_32 = 0u64;
                    let mut validation_before_max = 0u8;
                    let mut validation_after_max = 0u8;
                    for state in states {
                        let original = ssb_rom::filter_compensation::indices_to_rgba(
                            img.width, img.height, &source, state,
                        );
                        let candidate = ssb_rom::filter_compensation::indices_to_rgba(
                            img.width, img.height, &optimized, state,
                        );
                        let a = ssb_rom::filter_compensation::measure_samples(
                            &original, &original, t.clamp_s, t.clamp_t, samples,
                        );
                        let b = ssb_rom::filter_compensation::measure_samples(
                            &original, &candidate, t.clamp_s, t.clamp_t, samples,
                        );
                        before += a.squared_error;
                        after += b.squared_error;
                        baseline_large += a.above_8;
                        baseline_samples += a.samples;
                        baseline_max = baseline_max.max(a.max);
                        better_large |= b.above_8 < a.above_8;
                        nonregressing &=
                            b.max <= a.max.saturating_add(8) && b.above_32 <= a.above_32;
                        let va = ssb_rom::filter_compensation::measure_samples_policy(
                            &original,
                            &original,
                            t.clamp_s,
                            t.clamp_t,
                            validation_samples,
                            alpha_policy,
                        );
                        let vb = ssb_rom::filter_compensation::measure_samples_policy(
                            &original,
                            &candidate,
                            t.clamp_s,
                            t.clamp_t,
                            validation_samples,
                            alpha_policy,
                        );
                        validation_before += va.squared_error;
                        validation_after += vb.squared_error;
                        validation_before_32 += va.above_32;
                        validation_after_32 += vb.above_32;
                        validation_before_max = validation_before_max.max(va.max);
                        validation_after_max = validation_after_max.max(vb.max);
                        nonregressing &= validation_preserved(va, vb, alpha_policy);
                    }
                    let accepted = !validation_samples.is_empty()
                        && before > 0
                        && after * 100 <= before * 95
                        && better_large
                        && nonregressing;
                    let high_error = baseline_samples > 0
                        && baseline_large * 100 >= baseline_samples
                        && baseline_max >= 16;
                    let direct_sse = if !accepted && high_error {
                        let mut sum = 0u64;
                        for state in states {
                            let original = ssb_rom::filter_compensation::indices_to_rgba(
                                img.width, img.height, &source, state,
                            );
                            let baseline = ssb_rom::filter_compensation::measure_samples(
                                &original, &original, t.clamp_s, t.clamp_t, samples,
                            )
                            .squared_error;
                            sum += ssb_rom::filter_compensation::solve_samples(
                                &original,
                                t.clamp_s,
                                t.clamp_t,
                                samples,
                                alpha_policy,
                            )
                            .map_or(baseline, |s| s.compensated.squared_error.min(baseline));
                        }
                        Some(sum)
                    } else {
                        None
                    };
                    let pixels = img.width as i64 * img.height as i64;
                    let direct_extra_level0 = pixels * 4 * states.len() as i64
                        - (pixels * psm.bits() as i64).div_euclid(8);
                    eprintln!("animated-filter-comp file={} offset={:#X} palettes={} baseline_sse={} indexed_sse={} accepted={} high_error={} direct_sse={:?} direct_extra_level0={} format={:?} memory_delta=0 train_samples={} validation_samples={} validation_baseline_sse={} validation_candidate_sse={} validation_baseline_above32={} validation_candidate_above32={} validation_baseline_max={} validation_candidate_max={}", t.data_file.map_or(src.home.id,u32::from), t.data_offset, states.len(), before, after, accepted, high_error, direct_sse, direct_extra_level0, psm, samples.len(), validation_samples.len(), validation_before, validation_after, validation_before_32, validation_after_32, validation_before_max, validation_after_max);
                    let indices = if accepted { &optimized } else { &source };
                    if residuals::enabled() {
                        record(
                            &ssb_rom::filter_compensation::indices_to_rgba(
                                img.width, img.height, indices, &palette,
                            ),
                            psm,
                            if accepted {
                                "animated-joint-index"
                            } else {
                                "animated-rejected"
                            },
                            true,
                            Some(residuals::Animated {
                                source_indices: source.clone(),
                                final_indices: indices.clone(),
                                states: states.to_vec(),
                            }),
                        );
                    }
                    if accepted {
                        if let Some(out) = compensated_out.as_deref_mut() {
                            *out = true;
                        }
                    }
                    return Some(psp::pack_mipped_indices(
                        &img, psm, &palette, swizzle, indices, states,
                    ));
                } else {
                    // An incomplete table cannot define a safe joint legal
                    // index set. Retain the source indices exactly instead
                    // of falling through to frame-zero RGB quantization.
                    record(
                        &img,
                        psm,
                        "animated-incomplete-table",
                        false,
                        Some(residuals::Animated {
                            source_indices: source.clone(),
                            final_indices: source.clone(),
                            states: vec![palette.clone()],
                        }),
                    );
                    return Some(psp::pack_mipped_indices(
                        &img,
                        psm,
                        &palette,
                        swizzle,
                        &source,
                        std::slice::from_ref(&palette),
                    ));
                }
            }
        }
        let solved = if train_samples.is_empty() {
            ssb_rom::filter_compensation::solve(&img, t.clamp_s, t.clamp_t, alpha_policy)
        } else {
            ssb_rom::filter_compensation::solve_samples(
                &img,
                t.clamp_s,
                t.clamp_t,
                train_samples,
                alpha_policy,
            )
        };
        let Some(solved) = solved else {
            texgen_report(&img, false, "no-solve", None);
            record(&img, psm, "no-solve", true, None);
            return Some(psp::pack_mipped(&img, psm, &palette, swizzle));
        };
        // Requirement 4: quantizing to the nearest existing palette/5551
        // entry can move a texel's alpha byte again (to whatever the
        // nearest immutable entry happens to carry) even though
        // `solve_samples` already proved the pre-quantization candidate
        // silhouette-safe -- so a cutout texture needs this checked again
        // against the actual final representation, not just the float solve.
        let cutout_gate = match alpha_policy {
            ssb_rom::filter_compensation::AlphaPolicy::Cutout {
                greater_or_equal,
                threshold,
            } => Some((greater_or_equal, threshold)),
            _ => None,
        };
        // Texgen falls back to dense full-tile coverage the same way
        // `solve`/`measure` do internally when given no explicit coverage.
        let dense_coverage: Vec<[i32; 2]>;
        let mismatch_coverage: &[[i32; 2]] = if train_samples.is_empty() {
            dense_coverage = (0..img.height as i32 * 32)
                .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                .flat_map(|tq| {
                    (0..img.width as i32 * 32)
                        .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                        .map(move |sq| [sq, tq])
                })
                .collect();
            &dense_coverage
        } else {
            train_samples
        };
        let silhouette_preserved = |candidate: &ssb_rom::texture::Rgba8| {
            cutout_gate.is_none_or(|(greater_or_equal, threshold)| {
                ssb_rom::filter_compensation::count_cutout_mismatches(
                    &img,
                    candidate,
                    t.clamp_s,
                    t.clamp_t,
                    mismatch_coverage,
                    greater_or_equal,
                    threshold,
                ) == 0
            })
        };
        let mut direct_refined = (psm == psp::Psm::Psm8888).then(|| {
            ssb_rom::filter_compensation::refine_integer(
                &img,
                &solved.rgba,
                ssb_rom::filter_compensation::IntegerFormat::Rgba8888,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
                alpha_policy,
            )
        });
        let mut chosen = direct_refined.as_ref().unwrap_or(&solved.rgba);
        let mut format = psm;
        let quantized;
        let mut integer_before_metrics = None;
        let mut integer_after_metrics = None;
        let mut used_index_optimization = false;
        let mut nearest_sse_dbg: i64 = -1;
        let mut optimized_sse_dbg: i64 = -1;
        let mut nearest_max_dbg: i64 = -1;
        let mut optimized_max_dbg: i64 = -1;
        let mut nearest_above8_dbg: i64 = -1;
        let mut optimized_above8_dbg: i64 = -1;
        let mut nearest_above32_dbg: i64 = -1;
        let mut optimized_above32_dbg: i64 = -1;
        if psm.is_paletted() {
            // Nearest-palette-color quantization minimizes per-texel color
            // distance, not the actual filtered output error: neighbouring
            // texels participate in the GE's bilinear reconstruction, so the
            // nearest entry in isolation is not necessarily the index that
            // minimizes what a sample actually reads. `optimize_palette_indices`
            // coordinate-descends over the exact measured objective instead,
            // seeded from this same nearest assignment; it is only kept if it
            // measures no worse on every metric (requirement 10 -- never trade
            // away a metric the plain nearest quantization already had).
            let nearest = ssb_rom::filter_compensation::quantize_to_palette(&solved.rgba, &palette);
            let nearest_metrics = ssb_rom::filter_compensation::measure_samples(
                &img,
                &nearest,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            );
            let max_index = if psm == psp::Psm::PsmT4 {
                16
            } else {
                palette.len()
            };
            let optimized = ssb_rom::filter_compensation::optimize_palette_indices(
                &img,
                &solved.rgba,
                &palette,
                max_index,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            );
            let optimized_metrics = ssb_rom::filter_compensation::measure_samples(
                &img,
                &optimized,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            );
            let nearest_validation = ssb_rom::filter_compensation::measure_samples_policy(
                &img,
                &nearest,
                t.clamp_s,
                t.clamp_t,
                validation_samples,
                alpha_policy,
            );
            let optimized_validation = ssb_rom::filter_compensation::measure_samples_policy(
                &img,
                &optimized,
                t.clamp_s,
                t.clamp_t,
                validation_samples,
                alpha_policy,
            );
            nearest_sse_dbg = nearest_metrics.squared_error as i64;
            optimized_sse_dbg = optimized_metrics.squared_error as i64;
            nearest_max_dbg = nearest_metrics.max as i64;
            optimized_max_dbg = optimized_metrics.max as i64;
            nearest_above8_dbg = nearest_metrics.above_8 as i64;
            optimized_above8_dbg = optimized_metrics.above_8 as i64;
            nearest_above32_dbg = nearest_metrics.above_32 as i64;
            optimized_above32_dbg = optimized_metrics.above_32 as i64;
            // Requirement 5 (RE-306), carried into index optimization: raw
            // RGBA SSE treats alpha and RGB as equally-weighted channels, so
            // a Translucent candidate can hold or improve raw SSE while
            // making the alpha-weighted *visible* result worse -- e.g.
            // trading a solid opaque-looking edge texel for one whose alpha
            // drops, fragmenting a small sprite's silhouette into holes
            // against the background even though nothing in the raw metric
            // caught it. `solve_samples` already gates its own continuous
            // solve on this; the index-optimization stage needs the same
            // gate, since it is a second, independent place alpha can move.
            let visible_ok = match alpha_policy {
                ssb_rom::filter_compensation::AlphaPolicy::Translucent => {
                    optimized_metrics.visible_squared_error <= nearest_metrics.visible_squared_error
                }
                _ => true,
            };
            let optimized_not_worse = optimized_metrics.squared_error
                <= nearest_metrics.squared_error
                && optimized_metrics.max <= nearest_metrics.max
                && optimized_metrics.above_8 <= nearest_metrics.above_8
                && optimized_metrics.above_32 <= nearest_metrics.above_32
                && validation_preserved(nearest_validation, optimized_validation, alpha_policy)
                && visible_ok;
            let (qm, candidate) = if optimized_not_worse {
                used_index_optimization = true;
                (optimized_metrics, optimized)
            } else {
                (nearest_metrics, nearest)
            };
            quantized = candidate;
            if qm.squared_error * 100 <= solved.baseline.squared_error * 95
                && qm.above_8 < solved.baseline.above_8
                && silhouette_preserved(&quantized)
            {
                chosen = &quantized;
            } else {
                // Promotion requires >=25% measured SSE improvement and is
                // capped at 64 KiB extra per immutable variant.
                let old_bytes = (img.width * img.height * psm.bits() as u32 / 8) as i64
                    + (palette.len() * 4) as i64;
                let new_bytes = (img.width * img.height * 4) as i64;
                if solved.compensated.squared_error * 4 > solved.baseline.squared_error * 3
                    || new_bytes - old_bytes > 65536
                {
                    texgen_report(
                        &img,
                        false,
                        "no-promotion",
                        Some((
                            solved.baseline.squared_error,
                            solved.compensated.squared_error,
                        )),
                    );
                    record(&img, psm, "no-promotion", true, None);
                    return Some(psp::pack_mipped(&img, psm, &palette, swizzle));
                }
                format = psp::Psm::Psm8888;
                direct_refined = Some(ssb_rom::filter_compensation::refine_integer(
                    &img,
                    &solved.rgba,
                    ssb_rom::filter_compensation::IntegerFormat::Rgba8888,
                    t.clamp_s,
                    t.clamp_t,
                    mismatch_coverage,
                    alpha_policy,
                ));
                chosen = direct_refined.as_ref().unwrap();
            }
        } else if psm == psp::Psm::Psm5551 {
            let rounded = ssb_rom::filter_compensation::quantize_rgba5551(&solved.rgba);
            quantized = ssb_rom::filter_compensation::refine_integer(
                &img,
                &rounded,
                ssb_rom::filter_compensation::IntegerFormat::Rgba5551,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
                alpha_policy,
            );
            integer_before_metrics = Some(ssb_rom::filter_compensation::measure_samples(
                &img,
                &rounded,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            ));
            let qm = if train_samples.is_empty() {
                ssb_rom::filter_compensation::measure(&img, &quantized, t.clamp_s, t.clamp_t)
            } else {
                ssb_rom::filter_compensation::measure_samples(
                    &img,
                    &quantized,
                    t.clamp_s,
                    t.clamp_t,
                    train_samples,
                )
            };
            integer_after_metrics = Some(qm);
            if qm.squared_error * 100 <= solved.baseline.squared_error * 95
                && qm.above_8 < solved.baseline.above_8
                && qm.max <= solved.baseline.max.saturating_add(8)
                && silhouette_preserved(&quantized)
            {
                chosen = &quantized;
            } else {
                let delta = (img.width * img.height * 2) as i64;
                if solved.compensated.squared_error * 4 > solved.baseline.squared_error * 3
                    || delta > 65536
                {
                    texgen_report(
                        &img,
                        false,
                        "no-promotion",
                        Some((
                            solved.baseline.squared_error,
                            solved.compensated.squared_error,
                        )),
                    );
                    record(&img, psm, "no-promotion", true, None);
                    return Some(psp::pack_mipped(&img, psm, &palette, swizzle));
                }
                format = psp::Psm::Psm8888;
                direct_refined = Some(ssb_rom::filter_compensation::refine_integer(
                    &img,
                    &solved.rgba,
                    ssb_rom::filter_compensation::IntegerFormat::Rgba8888,
                    t.clamp_s,
                    t.clamp_t,
                    mismatch_coverage,
                    alpha_policy,
                ));
                chosen = direct_refined.as_ref().unwrap();
            }
        }
        if format == psp::Psm::Psm8888 {
            integer_before_metrics = Some(solved.compensated);
            integer_after_metrics = Some(ssb_rom::filter_compensation::measure_samples(
                &img,
                chosen,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            ));
        }
        let integer_fields = |m: Option<ssb_rom::filter_compensation::Metrics>| {
            m.map_or([-1; 4], |m| {
                [
                    m.squared_error as i64,
                    m.above_32 as i64,
                    m.above_8 as i64,
                    m.max as i64,
                ]
            })
        };
        let integer_before = integer_fields(integer_before_metrics);
        let integer_after = integer_fields(integer_after_metrics);
        let train_before = ssb_rom::filter_compensation::measure_samples_policy(
            &img,
            &img,
            t.clamp_s,
            t.clamp_t,
            mismatch_coverage,
            alpha_policy,
        );
        let train_after = ssb_rom::filter_compensation::measure_samples_policy(
            &img,
            chosen,
            t.clamp_s,
            t.clamp_t,
            mismatch_coverage,
            alpha_policy,
        );
        let validation_before = ssb_rom::filter_compensation::measure_samples_policy(
            &img,
            &img,
            t.clamp_s,
            t.clamp_t,
            validation_samples,
            alpha_policy,
        );
        let validation_after = ssb_rom::filter_compensation::measure_samples_policy(
            &img,
            chosen,
            t.clamp_s,
            t.clamp_t,
            validation_samples,
            alpha_policy,
        );
        let validation_accepted = !validation_samples.is_empty()
            && validation_preserved(validation_before, validation_after, alpha_policy)
            && cutout_gate.is_none_or(|(greater_or_equal, threshold)| {
                ssb_rom::filter_compensation::count_cutout_mismatches(
                    &img,
                    chosen,
                    t.clamp_s,
                    t.clamp_t,
                    validation_samples,
                    greater_or_equal,
                    threshold,
                ) == 0
            })
            && conservative_gate(chosen);
        texgen_report(
            chosen,
            validation_accepted,
            "holdout-gate",
            Some((
                solved.baseline.squared_error,
                solved.compensated.squared_error,
            )),
        );
        eprintln!("filter-comp file={} offset={:#X} {}x{} policy={:?} baseline mean={:.3} max={} >=8={:.2}% >=32={:.2}% visible_mean={:.3} visible_max={} compensated mean={:.3} max={} >=8={:.2}% >=32={:.2}% visible_mean={:.3} visible_max={} format={:?} memory_delta={} index_opt={} nearest_sse={} optimized_sse={} nearest_max={} optimized_max={} nearest_above8={} optimized_above8={} nearest_above32={} optimized_above32={} integer_before_sse={} integer_after_sse={} integer_before_above32={} integer_after_above32={} integer_before_above8={} integer_after_above8={} integer_before_max={} integer_after_max={} train_samples={} train_baseline_sse={} train_candidate_sse={} train_baseline_above32={} train_candidate_above32={} train_baseline_max={} train_candidate_max={} validation_samples={} validation_baseline_sse={} validation_candidate_sse={} validation_baseline_above32={} validation_candidate_above32={} validation_baseline_max={} validation_candidate_max={} validation_accepted={}",
            t.data_file.map_or(src.home.id,u32::from),t.data_offset,img.width,img.height,alpha_policy,
            solved.baseline.mean,solved.baseline.max,solved.baseline.percent_above_8(),solved.baseline.percent_above_32(),
            solved.baseline.visible_mean,solved.baseline.visible_max,
            solved.compensated.mean,solved.compensated.max,solved.compensated.percent_above_8(),solved.compensated.percent_above_32(),
            solved.compensated.visible_mean,solved.compensated.visible_max,format,
            (img.width*img.height*format.bits() as u32/8) as i64-(img.width*img.height*psm.bits() as u32/8) as i64,
            used_index_optimization, nearest_sse_dbg, optimized_sse_dbg,
            nearest_max_dbg, optimized_max_dbg, nearest_above8_dbg, optimized_above8_dbg,
            nearest_above32_dbg, optimized_above32_dbg,
            integer_before[0], integer_after[0], integer_before[1], integer_after[1],
            integer_before[2], integer_after[2], integer_before[3], integer_after[3],
            train_before.samples, train_before.squared_error, train_after.squared_error,
            train_before.above_32, train_after.above_32, train_before.max, train_after.max,
            validation_before.samples, validation_before.squared_error, validation_after.squared_error,
            validation_before.above_32, validation_after.above_32, validation_before.max,
            validation_after.max, validation_accepted);
        if !validation_accepted {
            record(&img, psm, "holdout-rejected", true, None);
            return Some(psp::pack_mipped(&img, psm, &palette, swizzle));
        }
        record(
            chosen,
            format,
            if format != psm {
                "promoted-rgba8888"
            } else if psm.is_paletted() && used_index_optimization {
                "indexed-optimized"
            } else if psm.is_paletted() {
                "indexed-nearest"
            } else if psm == psp::Psm::Psm5551 {
                "direct-rgba5551"
            } else {
                "direct-rgba8888"
            },
            true,
            None,
        );
        if let Some(out) = compensated_out.as_deref_mut() {
            *out = true;
        }
        Some(psp::pack_mipped(
            chosen,
            format,
            if format.is_paletted() { &palette } else { &[] },
            swizzle,
        ))
    };

    // Every format enters the same compensation pipeline (RE-313): indexed
    // formats with their palette, direct ones -- RGBA16 as `Psm5551`, RGBA32
    // and IA as `Psm8888`, and the RE-283 bypass as `Psm8888` -- without.
    if psm.is_paletted() {
        mipped(index_colours.clone())
    } else {
        mipped(Vec::new())
    }
}

/// TMEM contents past a short `G_LOADTLUT`, measured on the original game
/// (RE-313).
///
/// `G_LOADTLUT` writes exactly `count` entries (`angrylion-rdp-plus`'s
/// `loading_pipeline` advances one entry per span step). An index past them
/// reads whatever an earlier load left in that TMEM slot. That depends on
/// draw order, so no static rule gives it; RE-313 replayed the real frame
/// display lists through `angrylion-rdp-plus`'s TMEM loader instead. Each
/// row is `(palette file, palette offset, loaded entries, &[(slot, entry)])`.
/// The entry is copy 0 of the slot's four TMEM copies: the RDP samples the
/// other copies for other bilinear taps, which a per-texel palette cannot
/// express.
const STALE_TLUT_ENTRIES: &[(u32, u32, u16, &[(u8, u16)])] = &[
    // Break the Targets direction arrows (Donkey Kong, Fox, Pikachu, Kirby):
    // slot 3 holds background file 119's last strip, `1091 1091 114F 114F`,
    // in every measured frame and camera position.
    (120, 0x598, 3, &[(3, 0x1091)]),
    (121, 0x6E0, 3, &[(3, 0x1091)]),
    (122, 0x2C0, 3, &[(3, 0x1091)]),
    (123, 0xAB8, 3, &[(3, 0x1091)]),
    // Opening-room book: slot 255 holds file 52's `0x42F0` wall texture,
    // `FE59 A391 FF9B FE59`, in every measured frame.
    (52, 0x9958, 255, &[(255, 0xFE59)]),
];

/// Extends a TLUT shorter than the indices its texture uses (RE-313).
///
/// A slot [`STALE_TLUT_ENTRIES`] records gets its measured value. Any other
/// slot is unresolved stale TMEM: it is filled with transparent black, the
/// value the decoder already gives it, so the packed texels match the
/// reference either way, and it is reported.
fn extend_short_tlut(
    src: Texels<'_>,
    file: &[u8],
    t: &ssb_rom::mesh::TextureRef,
    tlut: &mut Vec<u16>,
) {
    if !matches!(
        t.size,
        ssb_rom::texture::BitSize::Bits4 | ssb_rom::texture::BitSize::Bits8
    ) {
        return;
    }
    let Some(max) = decode_index_field(file, t).and_then(|i| i.into_iter().max()) else {
        return;
    };
    let need = max as usize + 1;
    if need <= tlut.len() {
        return;
    }
    let palette_file = t.palette_file.map_or(src.home.id, u32::from);
    let known = STALE_TLUT_ENTRIES
        .iter()
        .find(|(f, off, n, _)| {
            *f == palette_file && Some(*off) == t.palette_offset && *n == t.palette_entries
        })
        .map_or(&[][..], |row| row.3);
    let bank_base = if t.size == ssb_rom::texture::BitSize::Bits4 {
        u32::from(t.palette) * 16
    } else {
        0
    };
    for slot in tlut.len()..need {
        let absolute = bank_base + slot as u32;
        match known.iter().find(|(s, _)| u32::from(*s) == absolute) {
            Some(&(_, entry)) => tlut.push(entry),
            None => {
                eprintln!(
                    "stale-tlut-unresolved file={} offset={:#X} format={:?}/{:?} tlut={:?} palette={}:{:#X} entries={} slot={}",
                    t.data_file.map_or(src.home.id, u32::from),
                    t.data_offset,
                    t.format,
                    t.size,
                    t.tlut,
                    palette_file,
                    t.palette_offset.unwrap_or(0),
                    t.palette_entries,
                    absolute
                );
                tlut.push(0);
            }
        }
    }
}

/// Decodes the visible tile from a possibly padded ROM image buffer.
///
/// `G_LOADBLOCK` records source rows in 64-bit-word strides. The renderer
/// samples only `TextureRef::width` texels. The render tile's origin belongs
/// to texture-coordinate addressing, not the source image: `mesh::Builder`
/// has already rebased authored UVs for clamped tiles. Therefore this lowers
/// the physical source buffer from its top-left corner without applying that
/// origin a second time.
fn decode_texture(
    file: &[u8],
    t: &ssb_rom::mesh::TextureRef,
    tlut: &[u16],
) -> Option<ssb_rom::texture::Rgba8> {
    use ssb_rom::texture;

    let source_width = u32::from(t.source_width.max(t.width));
    let width = u32::from(t.width);
    let height = u32::from(t.height);
    let source_row = texture::data_len(source_width, 1, t.size);
    let need = source_row.checked_mul(height as usize)?;
    let source = file.get(t.data_offset as usize..t.data_offset as usize + need)?;
    // `tlut` already starts at the CI4 bank; the bank only reaches the
    // decoder for a CI texel read with the TLUT off (RE-313).
    let palette = if t.tlut.enabled() { 0 } else { t.palette };
    let image = texture::decode_lut(
        source,
        source_width,
        height,
        t.format,
        t.size,
        t.tlut,
        tlut,
        palette,
    )
    .ok()?;
    if source_width == width {
        return Some(image);
    }
    let mut tile = texture::Rgba8::new(width, height);
    for y in 0..height as usize {
        for x in 0..width as usize {
            tile.put(
                y * width as usize + x,
                image.get(y * source_width as usize + x),
            );
        }
    }
    Some(tile)
}

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
const EF_COMMON_EFFECTS2_MOBJ_PAIRS: &[(u32, u32, u32)] = &[
    (84, 0x2040, 0x1EA0), // FireSpark
    (84, 0x2760, 0x22B8), // CatchSwirl
    (84, 0x3398, 0x2F78), // ReflectBreak
    (84, 0x53E8, 0x4F08), // DeadExplode
    (84, 0x6D00, 0x6B40), // NessPKFlash
];

#[derive(Clone, Copy)]
struct EffectAsset {
    name: &'static str,
    file: u32,
    graph: u32,
}

/// Unique display-bearing scene graphs referenced by the 53 static `EFDesc`
/// records in `ef/efmanager.c`. Three controller-only descriptors have a NULL
/// display callback and no graph; four pairs of descriptors intentionally
/// share one graph. The remaining 46 are the source-named DObj effect surface
/// that can be checked independently of the still-unimplemented LBParticle
/// script runtime (RE-172).
const MANAGER_EFFECT_ASSETS: &[EffectAsset] = &[
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
const DIRECT_MANAGER_EFFECT_ASSETS: &[EffectAsset] = &[
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

const DIRECT_MANAGER_EFFECT_MOBJ_PAIRS: &[(u32, u32, u32)] = &[
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
fn object_mat_anims(
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

/// Verifies the original manager's display-bearing effect graphs survived the
/// ROM-to-pack pipeline and reports their stable object indices for the PSP
/// visual audit. This deliberately does not count LBParticle scripts: those
/// are a separate bytecode/texture-bank renderer and remain an explicit gap.
fn effects(path: &Path) -> Res {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = ssb_rom::pack::Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;
    let mut missing = Vec::new();
    let mut anim_errors = Vec::new();
    let mut animated = 0usize;
    let mut animated_nodes = 0usize;
    let mut total_tris = 0u32;
    let mut mat_animated = 0usize;
    let mut mat_animated_prims = 0usize;
    let mut mat_anim_errors = Vec::new();
    let mut particle_errors = Vec::new();

    for (effect_index, asset) in MANAGER_EFFECT_ASSETS.iter().enumerate() {
        let found = (0..pack.object_count()).find_map(|i| {
            let object = pack.object(i)?;
            (object.source_file == asset.file && object.source_offset == asset.graph)
                .then_some((i, object))
        });
        let Some((index, object)) = found else {
            missing.push(asset.name);
            println!(
                "  MISSING {:24} file {:3} @ 0x{:X}",
                asset.name, asset.file, asset.graph
            );
            continue;
        };
        let tris = (0..object.node_count)
            .filter_map(|n| pack.node(object.first_node + n))
            .filter_map(|n| (n.mesh != ssb_rom::pack::NodeDesc::NO_MESH).then_some(n.mesh))
            .filter_map(|m| pack.mesh(m))
            .map(|m| {
                (0..m.prim_count)
                    .filter_map(|p| pack.prim(m.first_prim + p))
                    .map(|p| p.index_count / 3)
                    .sum::<u32>()
            })
            .sum::<u32>();
        if tris == 0 {
            missing.push(asset.name);
        }
        total_tris += tris;
        println!(
            "  object {index:3}  file {:3} @ 0x{:05X}  {tris:4} tris  {}",
            asset.file, asset.graph, asset.name
        );

        if ssb_rom::effect::MANAGER_EFFECT_ANIM_JOINTS[effect_index].is_some() {
            let Some(anim) = pack.effect_anim(effect_index as u32) else {
                anim_errors.push(format!("{}: missing transform animation", asset.name));
                continue;
            };
            let Some(script) = pack.anim_script(&anim) else {
                anim_errors.push(format!("{}: missing animation bytes", asset.name));
                continue;
            };
            let mut player = ssb_rom::skeleton::StageAnimator::new();
            player.start(&pack, &anim);
            if player.joint_count() == 0 {
                anim_errors.push(format!("{}: no bound animation joints", asset.name));
                continue;
            }
            let mut failed = None;
            for frame in 0..240 {
                if let Err(error) = player.tick(script) {
                    failed = Some(format!("{} frame {frame}: {error}", asset.name));
                    break;
                }
                if player.ended() {
                    break;
                }
            }
            if let Some(error) = failed {
                anim_errors.push(error);
            } else {
                animated += 1;
                animated_nodes += player.joint_count();
            }
        } else if pack.effect_anim(effect_index as u32).is_some() {
            anim_errors.push(format!("{}: unexpected transform animation", asset.name));
        }

        // RE-175: replay every primitive's material animation the same way,
        // frame-4-deterministic alongside the transform tick above (the
        // shortest source stream there ends after frame 5, so the same
        // budget samples every effect while still active). `PrimDesc.
        // mat_anim` is per-primitive, so several distinct scripts can be
        // live on one object at once (Link Spin Attack's own colour script
        // is separate from its sprite/palette-bearing geometry).
        if ssb_rom::effect::MANAGER_EFFECT_MAT_ANIM_JOINTS[effect_index].is_some() {
            let mat_anims = object_mat_anims(&pack, &object);
            if mat_anims.is_empty() {
                // RE-178/RE-179: for these two specific effects, the source's
                // own `gcAddMatAnimJointAll` walk never reaches the one real
                // script their table carries either -- this is not a missed
                // attachment, so it is not counted as an error.
                if !ssb_rom::effect::MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS
                    .contains(&(asset.file, asset.graph))
                {
                    mat_anim_errors.push(format!("{}: no bound material animation", asset.name));
                }
            } else {
                let mut player = ssb_rom::skeleton::EffectMaterialAnimator::new();
                player.start(&pack, mat_anims.iter().copied());
                for _ in 0..4 {
                    player.tick(&pack);
                }
                mat_animated += 1;
                mat_animated_prims += mat_anims.len();
                // The concrete case this whole audit exists to prove: Link
                // Spin Attack's authored frame-zero primitive alpha of zero
                // (`refs/ssb-decomp-re/src/relocData/353_LinkSpecial2.c`'s
                // `0xFFFF6000` -> `0xFFFF60CC` ramp) must have measurably
                // ramped up by frame 4, not stayed blank.
                if asset.name == "LinkSpinAttack" {
                    let visible = mat_anims.iter().any(|&i| {
                        player
                            .resolved_colors(i)
                            .and_then(|c| c.prim)
                            .is_some_and(|rgba| rgba[3] > 0)
                    });
                    if !visible {
                        mat_anim_errors.push(
                            "LinkSpinAttack: primitive alpha still zero at frame 4".to_string(),
                        );
                    }
                }
            }
        } else if !object_mat_anims(&pack, &object).is_empty() {
            mat_anim_errors.push(format!("{}: unexpected material animation", asset.name));
        }
    }

    println!(
        "manager DObj effects: {}/{} renderable, {total_tris} triangles",
        MANAGER_EFFECT_ASSETS.len() - missing.len(),
        MANAGER_EFFECT_ASSETS.len()
    );
    println!("shared descriptors: CommonSpark, YoshiShield, NessPKThunderTrail, KirbyStar");
    println!("authored rest-invisible: NessPKFlash, SamusEntryPoint, LinkSpinAttack (AObjEvent32)");
    println!("transform animations: {animated}/35 replayable, {animated_nodes} bound node(s)");
    println!(
        "material animations: {mat_animated}/26 replayable, {mat_animated_prims} bound primitive script(s)"
    );
    println!("  source-unreachable (not errors, RE-178/RE-179): ImpactWave, MBallThrown");
    println!(
        "material variant selection: DeadExplode player 0 (DeadExplode1), \
         MBallThrown left-facing table (RE-175)"
    );
    println!("controller-only descriptors: DamageSpawnOrbs, DamageSpawnSparks, DamageSpawnMDust");
    if (
        pack.particle_bank_count(),
        pack.particle_script_count(),
        pack.particle_texture_count(),
    ) != (9, 160, 65)
    {
        particle_errors.push(format!(
            "wrong LBParticle counts: {}/{}/{}",
            pack.particle_bank_count(),
            pack.particle_script_count(),
            pack.particle_texture_count()
        ));
    }
    let mut particle_frames = 0u32;
    for bank_index in 0..pack.particle_bank_count() {
        let Some(bank) = pack.particle_bank(bank_index) else {
            particle_errors.push(format!("bank {bank_index}: missing descriptor"));
            continue;
        };
        for script_index in bank.first_script..bank.first_script + bank.script_count {
            let Some(script) = pack.particle_script(script_index) else {
                particle_errors.push(format!("bank {bank_index}: missing script {script_index}"));
                continue;
            };
            let Some(bytecode) = pack.particle_bytecode(&script) else {
                particle_errors.push(format!("script {script_index}: missing bytecode"));
                continue;
            };
            if let Err(error) = ssb_rom::particle::inspect_bytecode(bytecode) {
                particle_errors.push(format!("script {script_index}: {error:?}"));
            }
        }
        for texture_index in bank.first_texture..bank.first_texture + bank.texture_count {
            let Some(series) = pack.particle_texture(texture_index) else {
                particle_errors.push(format!(
                    "bank {bank_index}: missing texture {texture_index}"
                ));
                continue;
            };
            particle_frames += series.frame_count;
            for frame in 0..series.frame_count {
                let index = series.first_frame.saturating_add(frame);
                let Some(texture) = pack.texture(index) else {
                    particle_errors.push(format!("texture {texture_index}: missing frame {frame}"));
                    continue;
                };
                if pack.texture_data(&texture).is_none() {
                    particle_errors.push(format!("texture {texture_index}: empty frame {frame}"));
                }
            }
        }
    }
    if particle_frames != 246 {
        particle_errors.push(format!("wrong LBParticle frame count: {particle_frames}"));
    }
    println!(
        "LBParticle pack: {}/9 banks, {}/160 scripts, {}/65 texture series, {particle_frames}/246 frames",
        pack.particle_bank_count(),
        pack.particle_script_count(),
        pack.particle_texture_count()
    );
    println!("LBParticle runtime/renderer: not implemented");

    if !anim_errors.is_empty() {
        for error in &anim_errors {
            eprintln!("animation error: {error}");
        }
    }
    if !mat_anim_errors.is_empty() {
        for error in &mat_anim_errors {
            eprintln!("material animation error: {error}");
        }
    }
    if !particle_errors.is_empty() {
        for error in &particle_errors {
            eprintln!("particle error: {error}");
        }
    }
    if missing.is_empty()
        && anim_errors.is_empty()
        && mat_anim_errors.is_empty()
        && particle_errors.is_empty()
    {
        Ok(())
    } else {
        Err(format!(
            "{} manager effect object(s) missing or empty, {} animation error(s), \
             {} material animation error(s), {} particle error(s)",
            missing.len(),
            anim_errors.len(),
            mat_anim_errors.len(),
            particle_errors.len()
        )
        .into())
    }
}

fn particles(path: &Path) -> Res {
    let (data, _) = load_rom(path)?;
    let mut script_total = 0usize;
    let mut texture_total = 0usize;
    let mut frame_total = 0usize;
    let mut decoded_total = 0usize;
    let mut bytecode_total = 0usize;
    let mut visible_total = 0usize;
    let mut invisible: Vec<String> = Vec::new();
    // RE-186: the frame-4 settle point is also draw_particle's own combine-
    // mode decision point (lbparticle.c:2057-2099) -- ENVCOLOR selects the
    // already-shipped (PRIM-ENV)*TEXEL+ENV blend, NOISE and DITHER select
    // combine/alpha-compare paths this project has declined (RE-183's own
    // doc comment), so census which of those a *visible* script actually
    // reaches rather than guessing from the flag opcodes' bare existence.
    let mut envcolor_count = 0usize;
    let mut noise_count = 0usize;
    let mut dither_count = 0usize;
    let mut alphablend_count = 0usize;
    // RE-187: does actually executing MAKESCRIPT/MAKERAND/MAKEID (RE-184/186
    // only ever decoded them) change the frame-4 visibility census? A script
    // classified "spawner script, never resolves its own texture" may still
    // put a *child* particle on screen by frame 4.
    let mut tree_rescued = 0usize;
    let mut tree_errors: Vec<String> = Vec::new();
    // RE-188: MAKEGENERATOR's own subsystem (still declined by every
    // SpawnSink above and by particle_tree) -- does generator::Generator
    // run every real archive-wide target to completion, or hit a real
    // decline (kind 2/vortex, or a truly unknown kind)?
    let mut generator_targets = 0usize;
    let mut generator_vortex_declines = 0usize;
    let mut generator_unknown_declines: Vec<String> = Vec::new();
    let mut generator_ever_spawned = 0usize;
    let mut generator_ever_visible = 0usize;

    println!("LBParticle banks");
    for &spec in ssb_rom::particle::BANKS {
        let (scripts, textures) = ssb_rom::particle::decode_bank(&data, spec)
            .map_err(|error| format!("{}: {error:?}", spec.name))?;
        let frames: usize = textures.iter().map(|texture| texture.images.len()).sum();
        // RE-184: RE-183's PSP proof of concept only checked script 0 render
        // visibility. Reproduce its exact deterministic settle point --
        // spawn fresh, tick to frame 4 with the same fixed seed -- for every
        // real script, so a script that never puts a pixel on screen is
        // counted rather than silently skipped by a manual spot check.
        for (index, script) in scripts.iter().enumerate() {
            let mut particle = ssb_rom::particle::Particle::spawn(script);
            let mut rng = ssb_rom::particle::Rng::new(1);
            let mut spawn_count = 0usize;
            for _ in 0..4 {
                spawn_count += particle.tick(&mut rng).map_or(0, |v| v.len());
            }
            let frame_count = textures
                .get(particle.state.texture_id as usize)
                .map_or(0, |t| t.images.len() as u32);
            if particle.state.visible(frame_count) {
                visible_total += 1;
                let flags = particle.state.flags;
                if flags & ssb_rom::particle::flag::ENVCOLOR != 0 {
                    envcolor_count += 1;
                }
                if flags & ssb_rom::particle::flag::NOISE != 0 {
                    noise_count += 1;
                }
                if flags & ssb_rom::particle::flag::DITHER != 0 {
                    dither_count += 1;
                }
                if flags & ssb_rom::particle::flag::ALPHABLEND != 0 {
                    alphablend_count += 1;
                }
            } else {
                // RE-184 traced every one of these by hand across 60 ticks
                // with a fixed seed to tell three real cases apart rather
                // than reporting one flat "invisible" count: a zero-frame
                // texture series never has anything to bind (`frame_count ==
                // 0`); a script that issues `MAKESCRIPT`/`MAKEGENERATOR`
                // this window is a pure spawner whose own draw the original
                // never uses (`lbGeneratorRun`'s job, not yet ported --
                // RE-182's own declined scope); anything else needs a fresh
                // per-script look before being assumed harmless.
                let reason = if frame_count == 0 {
                    "authored zero-frame texture series"
                } else if spawn_count > 0 {
                    "spawner script (issues MAKESCRIPT/MAKEGENERATOR, never resolves its own texture)"
                } else {
                    "unexplained -- needs individual inspection"
                };
                invisible.push(format!(
                    "{} script {index}: {reason} (size {:.3}, texture {} frames {frame_count})",
                    spec.name, particle.state.size, particle.state.texture_id
                ));

                // RE-187: this script's own root particle put nothing on
                // screen -- check whether a real spawn tree (MAKESCRIPT/
                // MAKERAND/MAKEID actually executed, same settle point)
                // does, rather than assuming "spawner" means "invisible".
                let mut tree = ssb_rom::particle::particle_tree::ParticleTree::spawn_root(script);
                let mut tree_rng = ssb_rom::particle::Rng::new(1);
                let mut tick_error = None;
                for _ in 0..4 {
                    if let Err(error) = tree.tick_frame(&scripts, &mut tree_rng) {
                        tick_error = Some(error);
                        break;
                    }
                }
                match tick_error {
                    Some(error) => tree_errors.push(format!(
                        "{} script {index}: spawn-tree error {error:?}",
                        spec.name
                    )),
                    None => {
                        let any_visible = tree.live().any(|(_, p)| {
                            let frame_count = textures
                                .get(p.state.texture_id as usize)
                                .map_or(0, |t| t.images.len() as u32);
                            p.state.visible(frame_count)
                        });
                        if any_visible {
                            tree_rescued += 1;
                            println!(
                                "  spawn-tree rescued: {} script {index} (ever spawned {} particles)",
                                spec.name,
                                tree.ever_spawned()
                            );
                        }
                    }
                }
            }
        }
        for texture in &textures {
            for (frame, image) in texture.images.iter().enumerate() {
                let palette = texture
                    .palettes
                    .get(if texture.flags & 1 != 0 { 0 } else { frame });
                let palette: Option<Vec<u16>> = palette.map(|bytes| {
                    bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|word| u16::from_be_bytes([word[0], word[1]]))
                        .collect()
                });
                ssb_rom::texture::decode(
                    image,
                    texture.width,
                    texture.height,
                    texture.format,
                    texture.size,
                    palette.as_deref(),
                )
                .map_err(|error| format!("{} texture frame {frame}: {error:?}", spec.name))?;
                decoded_total += 1;
            }
        }
        // RE-188: reachable MAKEGENERATOR targets, found the same way as
        // RE-186/187's own censuses -- run each of this bank's real scripts
        // far enough (2000 ticks) to reach every real MAKEGENERATOR call,
        // then run each distinct target as a `Generator` for up to 240
        // frames (seed 1) or until it ejects/errors.
        let mut targets = std::collections::BTreeSet::new();
        for script in &scripts {
            let mut particle = ssb_rom::particle::Particle::spawn(script);
            let mut rng = ssb_rom::particle::Rng::new(1);
            for _ in 0..2000 {
                if let Ok(spawns) = particle.tick(&mut rng) {
                    for s in spawns {
                        if s.is_generator {
                            targets.insert(s.script_id);
                        }
                    }
                }
                if !particle.state.alive {
                    break;
                }
            }
        }
        for id in targets {
            let Some(target_script) = scripts.get(id as usize) else {
                continue;
            };
            generator_targets += 1;
            let mut gen = ssb_rom::particle::generator::Generator::spawn(target_script);
            let mut rng = ssb_rom::particle::Rng::new(1);
            let mut spawned_any = false;
            let mut visible_any = false;
            for _ in 0..240 {
                if !gen.alive {
                    break;
                }
                match gen.tick(&mut rng) {
                    Ok(particles) => {
                        for p in particles {
                            spawned_any = true;
                            let frame_count = textures
                                .get(p.state.texture_id as usize)
                                .map_or(0, |t| t.images.len() as u32);
                            if p.state.visible(frame_count) {
                                visible_any = true;
                            }
                        }
                    }
                    Err(ssb_rom::particle::SimError::VortexUnsupported) => {
                        generator_vortex_declines += 1;
                        break;
                    }
                    Err(error) => {
                        generator_unknown_declines
                            .push(format!("{} script {id}: {error:?}", spec.name));
                        break;
                    }
                }
            }
            if spawned_any {
                generator_ever_spawned += 1;
            }
            if visible_any {
                generator_ever_visible += 1;
            }
        }
        let used_bytecode: usize = scripts
            .iter()
            .map(|script| {
                ssb_rom::particle::inspect_bytecode(script.bytecode)
                    .unwrap()
                    .used_bytes
            })
            .sum();
        println!(
            "  {:14} {:3} scripts  {:2} textures  {:3} frames  {:5} bytecode bytes",
            spec.name,
            scripts.len(),
            textures.len(),
            frames,
            used_bytecode
        );
        script_total += scripts.len();
        texture_total += textures.len();
        frame_total += frames;
        bytecode_total += used_bytecode;
    }
    println!(
        "total: {script_total} scripts, {texture_total} textures, {frame_total} frames decoded, {bytecode_total} bytecode bytes"
    );
    debug_assert_eq!(decoded_total, frame_total);
    println!(
        "frame-4 render visibility (seed 1, RE-183's settle point): {visible_total}/{script_total} visible"
    );
    for line in &invisible {
        println!("  invisible: {line}");
    }
    println!(
        "combine-mode census among visible scripts (lbparticle.c:2057-2099): \
         ENVCOLOR {envcolor_count}/{visible_total}, NOISE {noise_count}/{visible_total}, \
         DITHER {dither_count}/{visible_total}, ALPHABLEND {alphablend_count}/{visible_total}"
    );
    let invisible_total = script_total - visible_total;
    println!(
        "spawn-tree rescue among {invisible_total} root-invisible scripts: \
         {tree_rescued} become visible once MAKESCRIPT/MAKERAND/MAKEID actually spawn \
         (frame 4, seed 1), {} spawn-tree error(s)",
        tree_errors.len()
    );
    for line in &tree_errors {
        println!("  spawn-tree error: {line}");
    }
    println!(
        "MAKEGENERATOR targets: {generator_targets} real (seed 1, up to 240 frames each), \
         {generator_ever_spawned} ever spawn a particle, {generator_ever_visible} ever spawn a \
         visible one, {generator_vortex_declines} decline as vortex (kind 2), \
         {} decline as an unknown kind",
        generator_unknown_declines.len()
    );
    for line in &generator_unknown_declines {
        println!("  unknown generator kind: {line}");
    }
    Ok(())
}

struct Loaded {
    files: Vec<Option<ssb_rom::archive::File>>,
    graphs: BTreeMap<u32, Vec<ssb_rom::scene::SceneGraph>>,
    tables: ssb_rom::mobj::PartTables,
    stages: Vec<ssb_rom::stage::GroundData>,
}

fn load_all(archive: &Archive) -> Loaded {
    use ssb_rom::{mobj, scene};

    let files: Vec<Option<ssb_rom::archive::File>> = (0..archive.len() as u32)
        .map(|id| archive.load(id).ok())
        .collect();
    let mut graphs: BTreeMap<u32, Vec<scene::SceneGraph>> = files
        .iter()
        .flatten()
        .map(|f| (f.id, scene::find_scene_graphs(f)))
        .collect();
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
    let tables = mobj::PartTables::scan(files.iter().flatten(), |model, graph, table| {
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

    let mut tables = tables;
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
    // RE-077: the same category as file 86's still-blocked case (`PLAN.md`
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
    for &(file, graph, table) in &[
        // ITCommonData's NBumper ItemAttributes names both the DObj resource
        // and its MObjSub*** table as externs from ITCommonObject.  The scene
        // parser starts at the first actual DObjDesc record (0x7BE8), within
        // the source label's broad 0x7648 data block; the material table is
        // the exact linker target at 0x7488.  This replaces RE-061's former
        // 27-way demand-search ambiguity with a direct original relationship.
        (86u32, 0x7BE8u32, 0x7488u32), // NBumper ItemAttributes
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
        (342u32, 0x2258u32, 0x101Cu32), // PikachuSpecial3 -- gap_0x0000_sub_0x1018[8], decomp-documented "2 NULL slots + 6 pointers"
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

    Loaded {
        files,
        graphs,
        tables,
        stages,
    }
}

impl Loaded {
    /// The materials for each node of a graph at the default costume, or
    /// empty vectors when no record names its table.
    fn materials(
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
    fn materials_at(
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
                for (m, c) in chain.iter_mut().zip(per_node) {
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
                    // `PaletteID` (RE-096: 45% of real fighter costume
                    // scripts carry one) selects which of
                    // `MObjSub.palettes[]` this costume actually wears --
                    // `m.palette` otherwise only ever holds `palettes[0]`
                    // (`read_material`'s own scope), silently costume 0's
                    // choice for every other costume. `objdisplay.c` reads
                    // it back as `palettes[(s32)mobj->palette_id]`, the same
                    // cast `colors_at` already applied.
                    if let Some(id) = c.palette_id.filter(|&id| id >= 0) {
                        if let Some(ptrs) =
                            ssb_rom::mobj::read_palettes(file, m.at, id as usize + 1)
                        {
                            if let Some(&p) = ptrs.get(id as usize) {
                                m.palette = Some(p);
                            }
                        }
                    }
                }
            }
        }
        nodes
    }
}

/// Which costume the pack is built for.
///
/// A fighter's colours are per-costume and the game picks at match start; the
/// converter has to pick one. Zero is the default the character select opens
/// on — Mario in red, Luigi in green.
const DEFAULT_COSTUME: f32 = 0.0;

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
const FIGHTER_COSTUME_COUNTS: &[(u32, u32)] = &[
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

fn fighter_costume_count(file: u32) -> u32 {
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
fn file_meshes(loaded: &Loaded, file: &ssb_rom::archive::File) -> Vec<ssb_rom::mesh::Mesh> {
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
        let plan = plan_draw_order(graph, &resolver);
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
    let all = ssb_rom::scan::find_root_display_lists(file);
    let called: BTreeSet<u32> = all.iter().flat_map(|d| d.referenced_lists()).collect();
    for dl in all
        .iter()
        .filter(|d| !called.contains(&d.offset) && !claimed.contains(&d.offset))
    {
        if unconvertible_without_materials(&dl.commands, file) {
            continue;
        }
        out.extend(mesh::convert(&dl.commands, mesh::Source::of(file)));
    }
    out
}

/// Whether a discovered list needs material state this converter cannot supply.
///
/// A list that calls segment `0x0E` gets its texture, palette or colour from
/// the node's `MObj` chain. Discovery finds lists by scanning bytes, so it has
/// no node and no chain, and the converter correctly drops the binding rather
/// than guessing — leaving geometry whose palette never loads. Packing that
/// means shipping a surface we know draws wrong. Lists a scene graph names are
/// unaffected: they come with their chain (RE-047).
fn unconvertible_without_materials(
    cmds: &[ssb_rom::dl::Cmd],
    file: &ssb_rom::archive::File,
) -> bool {
    ssb_rom::mobj::demand(cmds, &file.data) > 0
}

/// Replays every packed stage animation twice — once out of the pack, once
/// straight off the archive — and requires the two to agree frame for frame.
///
/// This is the check the fighter animations have had since RE-036 and stage
/// ones did not. It is not testing the decoder, which RE-050 already replayed
/// against the ROM; it is testing everything the *packing* added between the
/// archive and the device: the script offsets, the node indices, the copied
/// blob and the table row a stage's animation ends up in. That is exactly where
/// the one real bug lived — 35 rows written ahead of the fighter block, which
/// every existing check passed straight over (RE-051).
fn verify_stage_anims_against_pack(loaded: &Loaded, pack_path: &Path) -> Res {
    use ssb_rom::objanim::StageJoint;
    use ssb_rom::skeleton::StageAnimator;

    const FRAMES: u32 = 240;

    let bytes = fs::read(pack_path).map_err(|e| format!("{}: {e}", pack_path.display()))?;
    let pack = ssb_rom::pack::Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;

    let (mut stages, mut joints, mut compared, mut mismatched) = (0usize, 0usize, 0u64, 0u64);
    let mut worst = 0.0f32;

    for stage_index in 0..pack.stage_count() {
        let Some(a) = pack.stage_anim(stage_index) else {
            continue;
        };
        let Some(packed_script) = pack.anim_script(&a) else {
            return Err(format!("stage {stage_index}: packed script bytes missing").into());
        };
        let Some(file) = loaded
            .files
            .get(a.source_file as usize)
            .and_then(Option::as_ref)
        else {
            return Err(
                format!("stage {stage_index}: source file {} absent", a.source_file).into(),
            );
        };
        // The blob is meant to be the archive file verbatim; a differing byte
        // would make every offset below agree while the data underneath moved.
        if packed_script != file.data.as_slice() {
            return Err(format!(
                "stage {stage_index}: packed bytes differ from archive file {}",
                a.source_file
            )
            .into());
        }
        stages += 1;

        let mut from_pack = StageAnimator::new();
        from_pack.start(&pack, &a);

        // The same joints, rebuilt from the archive rather than read out of the
        // pack, so a wrong offset in the table shows up as a diverging pose.
        // Built by filtering the table the same way `StageAnimator::start`
        // does, because it skips unusable rows — indexing `first_joint + i`
        // against `joint(i)` would silently pair different joints.
        let mut direct: Vec<(u32, StageJoint, ssb_rom::figatree::JointPose)> = Vec::new();
        for i in 0..a.joint_count {
            if direct.len() == ssb_rom::skeleton::MAX_STAGE_JOINTS {
                break;
            }
            let Some(entry) = pack.anim_joint(a.first_joint + i) else {
                continue;
            };
            if entry.script == ssb_rom::pack::AnimJoint::NO_SCRIPT
                || entry.node == ssb_rom::pack::AnimJoint::NO_NODE
            {
                continue;
            }
            let node = entry.node;
            let Some(rest) = pack.node(node) else {
                continue;
            };
            direct.push((
                node,
                StageJoint::start(entry.script, 0.0),
                ssb_rom::figatree::JointPose {
                    rotate: rest.rest_rotate,
                    translate: rest.rest_translate,
                    scale: rest.rest_scale,
                },
            ));
            joints += 1;
        }
        if direct.len() != from_pack.joint_count() {
            return Err(format!(
                "stage {stage_index}: rebuilt {} joints, animator loaded {}",
                direct.len(),
                from_pack.joint_count()
            )
            .into());
        }

        for _ in 0..FRAMES {
            if let Err(e) = from_pack.tick(packed_script) {
                return Err(format!(
                    "stage {stage_index} (file {}): pack replay failed: {e}",
                    a.source_file
                )
                .into());
            }
            for (node, j, pose) in direct.iter_mut() {
                if let Err(e) = j.tick(&file.data, 1.0, pose) {
                    return Err(format!(
                        "stage {stage_index} (file {}) node {node}: archive replay failed: {e}",
                        a.source_file
                    )
                    .into());
                }
            }
            for (i, (node, _, want)) in direct.iter().enumerate() {
                let (got_node, got) = from_pack.joint(i).expect("in range");
                if got_node != *node {
                    return Err(format!(
                        "stage {stage_index} joint {i}: pack says node {got_node}, archive {node}"
                    )
                    .into());
                }
                for (g, w) in got
                    .rotate
                    .iter()
                    .chain(&got.translate)
                    .chain(&got.scale)
                    .zip(want.rotate.iter().chain(&want.translate).chain(&want.scale))
                {
                    compared += 1;
                    let d = (g - w).abs();
                    if d > worst {
                        worst = d;
                    }
                    if d > 0.0 {
                        mismatched += 1;
                    }
                }
            }
        }
    }

    println!(
        "\nstage animations replayed from {}: {stages} stage(s), {joints} joint(s)",
        pack_path.display()
    );
    println!("  {compared} pose value(s) compared against the archive");
    if mismatched == 0 {
        println!("            every packed pose matches the archive exactly");
    } else {
        return Err(format!("{mismatched} pose value(s) differ, worst {worst}").into());
    }
    Ok(())
}

/// Every floor segment of a stage, in the form the collision query wants.
///
/// The adapter lives here rather than in either crate because Layer A
/// (`ssb-game`) must not know the pack format and `ssb-rom` must not know the
/// game logic. The PSP build will need its own copy of these six lines; that
/// is the price of the layering, and it is cheaper than a shared type that
/// drags one crate into the other.
fn floor_segments(
    pack: &ssb_rom::pack::Pack,
    stage: &ssb_rom::pack::StageDesc,
) -> Vec<(u16, ssb_game::collision::Segment)> {
    use ssb_rom::pack::line_kind;

    let mut out = Vec::new();
    for line in pack.stage_lines(stage) {
        if line.kind != line_kind::FLOOR {
            continue;
        }
        let points: Vec<_> = pack.line_vertices(&line).collect();
        for pair in points.windows(2) {
            out.push((
                line.id,
                ssb_game::collision::Segment {
                    x1: pair[0].x,
                    y1: pair[0].y,
                    x2: pair[1].x,
                    y2: pair[1].y,
                    // The original reports the flags of the segment's first
                    // vertex through `stand_coll_flags`.
                    flags: pair[0].flags,
                },
            ));
        }
    }
    out
}

/// Runs the collision query against every stage in a built pack.
///
/// This is the end-to-end check: ROM -> extractor -> pack -> reader -> query.
/// Each stage's player spawns are dropped straight down and should land, since
/// the game places a spawn just above the surface it starts on. The margin is
/// the tell: across the archive almost every spawn comes to rest 3 or 4 units
/// below where it started, which no accident of geometry would produce.
///
/// A miss is not automatically a bug. Lines owned by a moving group are stored
/// in that group's own space and the runtime offsets them by the group's
/// `DObj` before testing; we have no group transforms yet, so those lines are
/// tested where they rest. That is why this reports rather than fails.
fn collide(path: &Path, opts: &[&str]) -> Res {
    use ssb_engine::math::Vec2;
    use ssb_game::collision::{check_floor, flags};

    let mut only: Option<u32> = None;
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--stage" => only = Some(parse_id(it.next().ok_or("--stage needs an index")?)?),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = ssb_rom::pack::Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;

    let mut landed = 0usize;
    let mut missed = 0usize;
    let mut stages_clean = 0usize;
    let mut stages_seen = 0usize;

    for i in 0..pack.stage_count() {
        if only.is_some_and(|s| s != i) {
            continue;
        }
        let Some(stage) = pack.stage(i) else { continue };
        let segments = floor_segments(&pack, &stage);
        stages_seen += 1;

        // How many distinct groups own floor lines. A stage whose floors are
        // spread over many groups is the one where "tested at rest" can bite.
        let groups: BTreeSet<u16> = pack
            .stage_lines(&stage)
            .filter(|l| l.kind == ssb_rom::pack::line_kind::FLOOR)
            .map(|l| l.yakumono)
            .collect();

        println!(
            "stage {i:2}  file {}  {} floor segments in {} group(s)",
            stage.source_file,
            segments.len(),
            groups.len()
        );

        let mut clean = true;
        for player in 0..4u16 {
            let Some(spawn) = pack.spawn(&stage, player) else {
                continue;
            };
            // Straight down to the blast zone: if nothing is under the spawn,
            // the fighter would fall out of the stage on frame one.
            let from = Vec2::new(spawn.x as f32, spawn.y as f32);
            let to = Vec2::new(spawn.x as f32, stage.bounds.bottom as f32);
            match check_floor(segments.iter().copied(), from, to) {
                Some(hit) => {
                    landed += 1;
                    let mut what = String::new();
                    if hit.flags & flags::CLIFF != 0 {
                        what.push_str(" cliff");
                    }
                    if hit.flags & flags::PASS != 0 {
                        what.push_str(" pass");
                    }
                    println!(
                        "  P{}  spawn ({:6},{:6})  lands on line {:3} at y {:7.1}, \
                         {:4.0} below{what}",
                        player + 1,
                        spawn.x,
                        spawn.y,
                        hit.line,
                        hit.point.y,
                        from.y - hit.point.y
                    );
                }
                None => {
                    missed += 1;
                    clean = false;
                    println!(
                        "  P{}  spawn ({:6},{:6})  no floor beneath it in world space",
                        player + 1,
                        spawn.x,
                        spawn.y
                    );
                }
            }
        }
        stages_clean += usize::from(clean);
    }

    println!(
        "\n{stages_clean}/{stages_seen} stages catch every spawn \
         ({landed} landed, {missed} did not)"
    );
    if missed > 0 {
        println!(
            "a spawn with nothing under it means its platform is in a moving group's \
             own space; group transforms are not extracted yet"
        );
    }
    Ok(())
}

/// Prints every character's constants, extracted from the ROM.
///
/// `--verify` cross-checks the offsets in [`ssb_rom::fighter::FIGHTER_FILES`]
/// against the decompilation's own transcription of the same structs. It needs
/// `refs/ssb-decomp-re`, so it is a development check and not part of `check`.
///
/// The comparison is the point. An offset table is a claim about where 27
/// structs begin, and the cheapest way to be wrong is to be *almost* right —
/// a table off by one word still decodes into floats that look like numbers.
/// Forty-five fields matching values written down independently, for every
/// character, is not something a wrong offset survives.
fn fighters(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::fighter;

    let mut verify = false;
    let mut refs = PathBuf::from("refs/ssb-decomp-re/src/relocData");
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--verify" | "-v" => verify = true,
            "--refs" => {
                refs = it.next().ok_or("--refs needs a path")?.into();
                verify = true;
            }
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    println!(
        "{:<10} {:>4} {:>7}  {:>7} {:>6} {:>6} {:>6} {:>5} {:>5}  {:>4}",
        "fighter", "file", "attrs", "gravity", "tvel", "walk", "dash", "jsq", "jumps", "top"
    );

    let mut ok = 0usize;
    let mut implausible = Vec::new();
    let mut decoded = Vec::new();
    for entry in fighter::FIGHTER_FILES {
        let file = archive.load(entry.file)?;
        let f = fighter::decode_file(entry, &file)?;
        let a = &f.attributes;
        println!(
            "{:<10} {:>4} {:>#7x}  {:>7} {:>6} {:>6} {:>6} {:>5} {:>5}  {:>4}",
            entry.name,
            entry.file,
            entry.offset,
            a.gravity,
            a.tvel_base,
            a.walk_speed_mul,
            a.dash_speed,
            a.kneebend_anim_length,
            a.jumps_max,
            a.map_coll.top,
        );
        if a.looks_plausible() {
            ok += 1;
        } else {
            implausible.push(entry.name);
        }
        decoded.push((entry, f));
    }

    println!();
    println!("{ok}/{} decode to plausible values", decoded.len());
    if !implausible.is_empty() {
        return Err(format!("implausible attributes: {}", implausible.join(", ")).into());
    }

    if !verify {
        println!("(pass --verify to cross-check against the decompilation)");
        return Ok(());
    }

    if !refs.exists() {
        return Err(format!(
            "{} not found; --verify needs the decompilation checked out",
            refs.display()
        )
        .into());
    }

    let mut checked = 0usize;
    let mut fields = 0usize;
    let mut mismatches = Vec::new();
    for (entry, f) in &decoded {
        let Some(src) = find_reloc_source(&refs, entry.file)? else {
            mismatches.push(format!("{}: no relocData source found", entry.name));
            continue;
        };
        let want = parse_attr_literals(&src)?;
        if want.len() != fighter::SCALAR_COUNT {
            mismatches.push(format!(
                "{}: parsed {} literals, expected {}",
                entry.name,
                want.len(),
                fighter::SCALAR_COUNT
            ));
            continue;
        }
        let got = attr_scalars(&f.attributes);
        for (i, (name, w)) in want.iter().enumerate() {
            fields += 1;
            if (got[i] - w).abs() > 1e-6 * w.abs().max(1.0) {
                mismatches.push(format!(
                    "{}.{name}: rom {} vs decomp {w}",
                    entry.name, got[i]
                ));
            }
        }
        checked += 1;
    }

    println!("verified    {checked} fighters, {fields} fields against the decompilation");
    if mismatches.is_empty() {
        println!("            all agree");
        Ok(())
    } else {
        for m in mismatches.iter().take(20) {
            println!("  !! {m}");
        }
        Err(format!("{} field(s) disagree", mismatches.len()).into())
    }
}

/// The scalar head of [`ssb_rom::fighter::FighterAttributes`] as a flat array,
/// in the declaration order the C literal also uses.
fn attr_scalars(a: &ssb_rom::fighter::FighterAttributes) -> [f32; 45] {
    [
        a.size,
        a.walkslow_anim_length,
        a.walkmiddle_anim_length,
        a.walkfast_anim_length,
        a.throw_walkslow_anim_length,
        a.throw_walkmiddle_anim_length,
        a.throw_walkfast_anim_length,
        a.rebound_anim_length,
        a.walk_speed_mul,
        a.traction,
        a.dash_speed,
        a.dash_decel,
        a.run_speed,
        a.kneebend_anim_length,
        a.jump_vel_x,
        a.jump_height_mul,
        a.jump_height_base,
        a.jumpaerial_vel_x,
        a.jumpaerial_height,
        a.air_accel,
        a.air_speed_max_x,
        a.air_friction,
        a.gravity,
        a.tvel_base,
        a.tvel_fast,
        a.jumps_max as f32,
        a.weight,
        a.attack1_followup_frames,
        a.dash_to_run,
        a.shield_size,
        a.shield_break_vel_y,
        a.shadow_size,
        a.jostle_width,
        a.jostle_x,
        a.is_metallic as u32 as f32,
        a.cam_offset_y,
        a.closeup_camera_zoom,
        a.camera_zoom,
        a.camera_zoom_base,
        a.map_coll.top,
        a.map_coll.center,
        a.map_coll.bottom,
        a.map_coll.width,
        a.cliffcatch_coll.0,
        a.cliffcatch_coll.1,
    ]
}

/// Finds the decompilation's source for one archive file, named `<id>_<Name>.c`.
fn find_reloc_source(dir: &Path, file: u32) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let prefix = format!("{file}_");
    for e in fs::read_dir(dir)? {
        let e = e?;
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(".c") {
            return Ok(Some(fs::read_to_string(e.path())?));
        }
    }
    Ok(None)
}

/// Pulls the leading scalar initialisers out of an `FTAttributes` C literal.
///
/// Only the flat `value, /* name */` lines and the two small aggregates
/// (`map_coll`, `cliffcatch_coll`) are parsed; the first aggregate after those
/// ends the scan. Where the source branches on region, the US arm is taken,
/// matching the ROM this project targets.
fn parse_attr_literals(src: &str) -> Result<Vec<(String, f32)>, Box<dyn std::error::Error>> {
    let start = src
        .find("FTAttributes d")
        .ok_or("no FTAttributes literal in source")?;
    let body = &src[start..];
    let body = &body[body.find('{').ok_or("malformed literal")? + 1..];

    let mut out: Vec<(String, f32)> = Vec::new();
    // `None` outside any #if; otherwise the region the current arm is for.
    let mut arm: Option<bool> = None;
    for line in body.lines() {
        let s = line.trim();
        if s.starts_with("#if") {
            arm = Some(!s.contains("REGION_JP"));
            continue;
        }
        if s.starts_with("#else") {
            arm = arm.map(|keep| !keep);
            continue;
        }
        if s.starts_with("#endif") {
            arm = None;
            continue;
        }
        if arm == Some(false) {
            continue;
        }

        if let Some((value, name)) = split_initialiser(s) {
            if value.starts_with('{') {
                // The two aggregates we model; anything else ends the scan.
                if name != "map_coll" && name != "cliffcatch_coll" {
                    break;
                }
                let inner = value.trim_start_matches('{').trim_end_matches('}');
                for (i, part) in inner.split(',').enumerate() {
                    let part = part.trim();
                    if part.is_empty() {
                        continue;
                    }
                    out.push((format!("{name}[{i}]"), parse_c_float(part)?));
                }
            } else {
                match parse_c_float(value) {
                    Ok(v) => out.push((name.to_string(), v)),
                    // A non-numeric initialiser (an enum, a pointer) means we
                    // have run past the scalar head.
                    Err(_) => break,
                }
            }
        }
    }
    Ok(out)
}

/// Splits `value, /* name */` into its two halves.
fn split_initialiser(s: &str) -> Option<(&str, &str)> {
    let comment = s.find("/*")?;
    let value = s[..comment].trim().trim_end_matches(',').trim();
    let name = s[comment + 2..].trim().trim_end_matches("*/").trim();
    if value.is_empty() || name.is_empty() || name.contains(' ') {
        return None;
    }
    Some((value, name))
}

fn parse_c_float(s: &str) -> Result<f32, Box<dyn std::error::Error>> {
    match s {
        "TRUE" => return Ok(1.0),
        "FALSE" => return Ok(0.0),
        _ => {}
    }
    let t = s.trim_end_matches('f').trim_end_matches('F');
    t.parse::<f32>()
        .map_err(|_| format!("not a scalar literal: {s}").into())
}

/// Runs a real fighter against every stage in a built pack.
///
/// `collide` asks whether a spawn has a floor under it. This asks the harder
/// question: does the ported physics, driven a tick at a time through the
/// ported collision process, actually leave a fighter standing there?
///
/// Three checks, each using something the pack does not contain:
///
/// * **Two solvers agree.** A fighter dropped from its spawn lands via the
///   swept line/line query; `project_floor` finds the same surface with a
///   straight vertical probe. These share no arithmetic, so agreement on 158
///   real spawns is not something a common bug would produce.
/// * **It stays put.** After landing, a second of ticks must not move it. Any
///   sign error in the landing snap or the ground update shows up as drift,
///   and drift compounds — a stage that holds still for 60 ticks is not
///   holding still by luck.
/// * **It cannot be launched through the stage.** Dropped from 3000 units up
///   at maximum knockback velocity, one tick's movement is longer than most
///   stages are wide. Only `mpProcessUpdateMain`'s substepping catches that.
///
/// Scripts a button-jump + held-stick input schedule against a stage's real
/// floor segments (native, no PSP/emulator needed) and reports where the
/// fighter lands each tick, plus whether a scripted jab connects
/// (`ssb_game::attack::spheres_overlap`) against a second fighter placed at
/// spawn 1. Built for RE-295 (`psp-game`'s Training-mode jump wiring, so the
/// jab could reach the real dummy spawn point on its platform) — deriving
/// the exact tick/stick schedule this way, against the same `Fighter::tick`/
/// collision code the PSP build uses, is what made a working headless
/// capture script tractable rather than a guess-and-check loop against the
/// emulator itself.
fn jumptest(path: &Path, opts: &[&str]) -> Res {
    use ssb_engine::input::ControllerState;
    use ssb_game::fighter::{Fighter, FighterKind};

    let mut stage_index = 0u32;
    let mut jump_tick = 10u32;
    let mut jump2_tick: Option<u32> = None;
    let mut attack_tick: Option<u32> = None;
    let mut stick_switch_tick = 0u32;
    let mut stick_release_tick = u32::MAX;
    let mut stick_x: i8 = -80;
    let mut ticks = 120u32;
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--stage" => stage_index = parse_id(it.next().ok_or("--stage needs an index")?)?,
            "--jump-tick" => {
                jump_tick = it
                    .next()
                    .ok_or("--jump-tick needs a value")?
                    .parse()
                    .map_err(|_| "bad --jump-tick")?
            }
            "--jump2-tick" => {
                jump2_tick = Some(
                    it.next()
                        .ok_or("--jump2-tick needs a value")?
                        .parse()
                        .map_err(|_| "bad --jump2-tick")?,
                )
            }
            "--stick-switch-tick" => {
                stick_switch_tick = it
                    .next()
                    .ok_or("--stick-switch-tick needs a value")?
                    .parse()
                    .map_err(|_| "bad --stick-switch-tick")?
            }
            "--stick-x" => {
                stick_x = it
                    .next()
                    .ok_or("--stick-x needs a value")?
                    .parse()
                    .map_err(|_| "bad --stick-x")?
            }
            "--attack-tick" => {
                attack_tick = Some(
                    it.next()
                        .ok_or("--attack-tick needs a value")?
                        .parse()
                        .map_err(|_| "bad --attack-tick")?,
                )
            }
            "--stick-release-tick" => {
                stick_release_tick = it
                    .next()
                    .ok_or("--stick-release-tick needs a value")?
                    .parse()
                    .map_err(|_| "bad --stick-release-tick")?
            }
            "--ticks" => {
                ticks = it
                    .next()
                    .ok_or("--ticks needs a value")?
                    .parse()
                    .map_err(|_| "bad --ticks")?
            }
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = ssb_rom::pack::Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;
    let stage = pack
        .stage(stage_index)
        .ok_or_else(|| format!("no stage {stage_index}"))?;
    let segments = floor_segments(&pack, &stage);
    let floors = || segments.iter().copied();

    // Matches `Play::at_spawn`: deliberately *not* settled onto the surface,
    // so the first couple of ticks are the real spawn-height fall, keeping
    // this scratch simulation's tick numbering aligned with psp-game's.
    let p1 = pack.spawn(&stage, 0).ok_or("no spawn 0")?;
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.pos = ssb_engine::math::Vec3::new(p1.x as f32, p1.y as f32, 0.0);

    let p2 = pack.spawn(&stage, 1).ok_or("no spawn 1")?;
    let mut dummy = Fighter::new(FighterKind::Mario, 1, 3);
    dummy.pos = ssb_engine::math::Vec3::new(p2.x as f32, p2.y as f32, 0.0);
    dummy.place_on_stage(floors());
    println!(
        "target (dummy spawn 1, settled): x={} y={}",
        dummy.pos.x, dummy.pos.y
    );
    for (id, seg) in &segments {
        if (seg.y1 as i32 - p2.y as i32).abs() < 60 || (seg.y2 as i32 - p2.y as i32).abs() < 60 {
            println!(
                "  floor line {id}: ({},{}) -> ({},{})",
                seg.x1, seg.y1, seg.x2, seg.y2
            );
        }
    }

    let mut jump_was_held = false;
    for tick in 0..ticks {
        let jump_held = tick == jump_tick || jump2_tick == Some(tick);
        let tapped = jump_held && !jump_was_held;
        let released = !jump_held && jump_was_held;
        jump_was_held = jump_held;

        let cur_stick_x = if tick >= stick_switch_tick && tick < stick_release_tick {
            stick_x
        } else {
            0
        };
        let mut buttons = ssb_engine::input::N64Buttons::default();
        if attack_tick == Some(tick) {
            buttons.set(ssb_engine::input::N64Buttons::A, true);
        }
        let input = ControllerState {
            buttons,
            stick_x: cur_stick_x,
            stick_y: 0,
            connected: true,
        };
        f.set_input(input, tapped, released);
        f.tick(floors);

        let hitbox_active = ssb_game::attack::jab1_hitbox_active(f.status.anim_frame)
            && f.status.status == ssb_game::status::Status::Attack11;
        let hitbox_pos = f.pos + ssb_game::attack::MARIO_JAB1_HITBOX.offset;
        let overlap = hitbox_active
            && ssb_game::attack::spheres_overlap(
                hitbox_pos,
                ssb_game::attack::MARIO_JAB1_HITBOX.radius,
                dummy.pos,
                ssb_game::attack::MARIO_HURTBOX_RADIUS,
            );

        let dx = f.pos.x - dummy.pos.x;
        let dy = f.pos.y - dummy.pos.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if overlap {
            println!("  *** HIT: jab connects at tick {tick} ***");
        }
        println!(
            "tick {tick:3}  pos=({:8.1},{:8.1})  grounded={:5}  dist={:7.1}  status={:?} anim_frame={:.1}",
            f.pos.x,
            f.pos.y,
            f.is_grounded(),
            dist,
            f.status.status,
            f.status.anim_frame,
        );
    }

    Ok(())
}

fn simulate(path: &Path, opts: &[&str]) -> Res {
    use ssb_engine::math::Vec2;
    use ssb_game::fighter::{Fighter, FighterKind};

    /// Ticks to fall from a spawn to its surface. Spawns sit single-digit
    /// units up, so this is generous by two orders of magnitude.
    const SETTLE_TICKS: u32 = 240;
    /// Ticks a landed fighter must hold still for.
    const REST_TICKS: u32 = 60;

    let mut only: Option<u32> = None;
    let mut verbose = false;
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--stage" => only = Some(parse_id(it.next().ok_or("--stage needs an index")?)?),
            "--verbose" | "-v" => verbose = true,
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = ssb_rom::pack::Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;

    let mut spawns = 0usize;
    let mut settled = 0usize;
    let mut agreed = 0usize;
    let mut at_rest = 0usize;
    let mut caught = 0usize;
    let mut substep_mattered = 0usize;
    let mut worst_drift = 0.0f32;
    let mut disagreements = Vec::new();

    for i in 0..pack.stage_count() {
        if only.is_some_and(|s| s != i) {
            continue;
        }
        let Some(stage) = pack.stage(i) else { continue };
        let segments = floor_segments(&pack, &stage);
        let floors = || segments.iter().copied();

        for player in 0..4u16 {
            let Some(spawn) = pack.spawn(&stage, player) else {
                continue;
            };
            spawns += 1;

            // The vertical probe: what the game thinks this spawn is above.
            let mut probe = Fighter::new(FighterKind::Mario, player as u8, 3);
            probe.pos = ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32, 0.0);
            if !probe.place_on_stage(floors()) {
                if verbose {
                    println!("  stage {i:2} P{}  nothing beneath the spawn", player + 1);
                }
                continue;
            }
            settled += 1;
            let expected = probe.floor.expect("placed").line;

            // The swept path: let it fall there under real gravity.
            let mut f = Fighter::new(FighterKind::Mario, player as u8, 3);
            f.pos = ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32, 0.0);
            let mut ticks = 0;
            while !f.is_grounded() && ticks < SETTLE_TICKS {
                f.tick(floors);
                ticks += 1;
            }
            if !f.is_grounded() {
                disagreements.push(format!(
                    "stage {i:2} P{}  still falling after {SETTLE_TICKS} ticks",
                    player + 1
                ));
                continue;
            }
            let landed = f.floor.expect("grounded").line;
            if landed == expected && (f.pos.y - probe.pos.y).abs() < 0.01 {
                agreed += 1;
            } else {
                disagreements.push(format!(
                    "stage {i:2} P{}  fell onto line {landed} at y {:.2}, \
                     but sits above line {expected} at y {:.2}",
                    player + 1,
                    f.pos.y,
                    probe.pos.y
                ));
            }

            // Now hold still.
            let rested_at = f.pos;
            for _ in 0..REST_TICKS {
                f.tick(floors);
            }
            let drift = (f.pos.x - rested_at.x)
                .abs()
                .max((f.pos.y - rested_at.y).abs());
            if drift > worst_drift {
                worst_drift = drift;
            }
            if drift == 0.0 && f.is_grounded() {
                at_rest += 1;
            } else {
                disagreements.push(format!(
                    "stage {i:2} P{}  drifted {drift} units while standing still",
                    player + 1
                ));
            }

            // And the tunnelling case: maximum knockback straight down, from
            // high enough that one tick's movement overshoots the stage.
            // Knockback goes in its own vector because gravity clamps
            // `vel_air` at terminal velocity — a launched fighter is the only
            // thing in the game that moves this fast.
            let mut launched = Fighter::new(FighterKind::Mario, player as u8, 3);
            let high = ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32 + 2000.0, 0.0);
            launched.pos = high;
            launched.physics.vel_knockback.y = -2500.0;
            launched.tick(floors);

            // Whether subdividing that movement changed the answer. The swept
            // query is exact along a straight line, so for a pure fall it
            // should not — this counts how often it does, rather than
            // assuming either way.
            let whole = ssb_game::collision::check_floor(
                floors(),
                Vec2::new(high.x, high.y),
                Vec2::new(high.x, high.y - 2500.0),
            );
            let stepped = launched.floor.map(|f| f.line);
            if whole.map(|h| h.line) != stepped {
                substep_mattered += 1;
            }

            if launched.is_grounded() {
                caught += 1;
            } else {
                disagreements.push(format!(
                    "stage {i:2} P{}  fell through the stage at maximum velocity, \
                     reaching y {:.0}",
                    player + 1,
                    launched.pos.y
                ));
            }

            if verbose {
                let s = probe.floor.expect("placed");
                println!(
                    "  stage {i:2} P{}  spawn ({:6},{:6})  lands line {landed:3} \
                     y {:8.2} after {ticks:3} ticks  {}",
                    player + 1,
                    spawn.x,
                    spawn.y,
                    f.pos.y,
                    surface_flags(s.flags)
                );
            }
        }
    }

    // A last sanity check that costs nothing: the projection and the swept
    // query must also agree about open air.
    let void = ssb_game::collision::project_floor(
        floor_segments(&pack, &pack.stage(0).ok_or("no stages in the pack")?)
            .iter()
            .copied(),
        Vec2::new(1.0e6, 0.0),
    );
    if void.is_some() {
        return Err("a point a million units off-stage found a floor under it".into());
    }

    println!("spawns      {spawns}");
    println!("settle      {settled} have a floor beneath them");
    println!("agree       {agreed}/{settled} land where the vertical probe says they should");
    println!("at rest     {at_rest}/{settled} do not move over {REST_TICKS} ticks (worst drift {worst_drift})");
    println!("substep     {caught}/{settled} caught when dropped at maximum knockback velocity");
    println!("            subdividing changed the outcome for {substep_mattered} of them");
    if substep_mattered == 0 {
        println!(
            "            expected while only floors are ported: the swept query is exact \n\
             \x20           along a straight line, so substepping earns its keep once a wall \n\
             \x20           can deflect a fighter mid-tick"
        );
    }

    if !disagreements.is_empty() {
        println!("\n{} to explain:", disagreements.len());
        for d in &disagreements {
            println!("  {d}");
        }
    }
    if settled < spawns {
        println!(
            "\n{} spawn(s) have nothing beneath them: their platforms belong to a moving \
             group and are stored in that group's own space, which is not extracted yet",
            spawns - settled
        );
    }
    Ok(())
}

/// Names the surface bits of a collision vertex's flags.
///
/// From `mpdef.h`: the upper byte carries `MAP_VERTEX_COLL_PASS` (1 << 14,
/// drop-through) and `MAP_VERTEX_COLL_CLIFF` (1 << 15, ledge-grabbable); the
/// lower byte is the `MPMaterial` that sets friction.
fn surface_flags(flags: u16) -> String {
    let mut s = String::new();
    if flags & (1 << 15) != 0 {
        s.push_str("cliff ");
    }
    if flags & (1 << 14) != 0 {
        s.push_str("pass ");
    }
    let material = flags & 0xFF;
    if material != 0 {
        s.push_str(&format!("mat{material}"));
    }
    s.trim_end().into()
}

/// Lists the `MPGroundData` headers — one per stage.
///
/// A stage's geometry, collision, bounds and music are spread over several
/// archive files and this struct is what ties them together, so this is the
/// entry point for anything that wants to load a stage rather than a lone
/// object. See `ssb_rom::stage`.
fn stages(path: &Path, opts: &[&str]) -> Res {
    let mut only_file: Option<u32> = None;
    let mut verbose = false;
    let mut pack_path: Option<PathBuf> = None;
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            "--lines" => verbose = true,
            "--pack" => pack_path = it.next().map(PathBuf::from),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    let loaded = load_all(&archive);

    let mut layers = 0usize;
    let (mut anim_ok, mut anim_frames) = (0usize, 0u64);
    let (mut anim_wild, mut anim_denormal) = (0u64, 0u64);
    let mut anim_running = 0usize;
    let mut anim_src_files: BTreeSet<u32> = BTreeSet::new();
    /// Long enough that a looping script proves it loops, and that one which
    /// runs off the end hits its `End` well inside the budget.
    const ANIM_REPLAY_FRAMES: u32 = 600;
    let mut anim_max = 0.0f32;
    let mut with_table = 0usize;
    let mut collision_ok = 0usize;
    let mut collision_bad = 0usize;
    // Stage material animation (RE-089): replay every script `p_matanim_joints`
    // resolves, the same way the joint-animation block above already replays
    // `anim_joints`. Cross-checks `matanim::resolve_scripts` (RE-089) and the
    // already-shipped `MaterialJoint` tick engine (RE-087) against RE-086's
    // independently-produced archive-wide census.
    const MATANIM_TRACK_NAMES: [&str; ssb_rom::matanim::TICK_TRACK_COUNT] = [
        "TextureIDCurrent",
        "TraU",
        "TraV",
        "ScaU",
        "ScaV",
        "TextureIDNext",
        "ScrU",
        "ScrV",
        "SetLFrac",
        "PaletteID",
        "PrimColor",
        "EnvColor",
        "BlendColor",
        "Light1Color",
        "Light2Color",
    ];
    let mut matanim_scripts = 0usize;
    let mut matanim_fail = 0usize;
    let mut matanim_categories = [0usize; ssb_rom::matanim::TICK_TRACK_COUNT];
    let mut colour_scripts: Vec<String> = Vec::new();
    let mut palette_examples: Vec<(u32, u32, u32, u32)> = Vec::new(); // (file, graph, script, entries needed)
                                                                      // RE-090: does the script-computed bound actually read a real
                                                                      // `palettes[]` array, not just a plausible-looking number?
    let (mut palette_reads_ok, mut palette_reads_fail, mut palette_reads_dup) =
        (0usize, 0usize, 0usize);
    for s in &loaded.stages {
        if only_file.is_some_and(|f| f != s.file) {
            continue;
        }
        println!("file {} @ 0x{:X}  bgm 0x{:X}", s.file, s.offset, s.bgm_id);
        println!(
            "  camera  top {:6} bottom {:6} right {:6} left {:6}",
            s.camera_bounds.top,
            s.camera_bounds.bottom,
            s.camera_bounds.right,
            s.camera_bounds.left
        );
        println!(
            "  map     top {:6} bottom {:6} right {:6} left {:6}",
            s.map_bounds.top, s.map_bounds.bottom, s.map_bounds.right, s.map_bounds.left
        );
        for l in &s.layers {
            layers += 1;
            let nodes = loaded
                .graphs
                .get(&l.graph.0)
                .and_then(|gs| gs.iter().find(|g| g.offset == l.graph.1))
                .map_or(0, |g| g.nodes.len());
            let table = match l.mobjsub_table {
                Some((f, at)) => {
                    with_table += 1;
                    format!("materials file {f} @ 0x{at:X}")
                }
                None => "no materials".into(),
            };
            if let Some((af, _)) = l.anim_joints {
                anim_src_files.insert(af);
            }
            if let Some((af, at)) = l.anim_joints {
                if af == l.graph.0 {
                    if let Some(f) = loaded.files.get(af as usize).and_then(Option::as_ref) {
                        let scripts = ssb_rom::objanim::joint_scripts(&f.data, at, nodes);
                        for (n, s) in scripts.iter().enumerate() {
                            let Some(script) = *s else { continue };
                            let mut j = ssb_rom::objanim::StageJoint::start(script, 0.0);
                            let mut pose = ssb_rom::figatree::JointPose {
                                rotate: [0.0; 3],
                                translate: [0.0; 3],
                                scale: [1.0; 3],
                            };
                            let mut frames = 0u32;
                            let mut err = None;
                            while frames < ANIM_REPLAY_FRAMES && !j.ended() {
                                if let Err(e) = j.tick(&f.data, 1.0, &mut pose) {
                                    err = Some(e);
                                    break;
                                }
                                frames += 1;
                                for v in
                                    pose.rotate.iter().chain(&pose.translate).chain(&pose.scale)
                                {
                                    // A desynchronised stream reads a command
                                    // word as a float, and a command word is a
                                    // small integer whose bit pattern is a
                                    // denormal — around 1e-35. Real keys are
                                    // radians, model units or scales.
                                    if !v.is_finite() {
                                        anim_wild += 1;
                                    } else if *v != 0.0 && v.abs() < 1e-20 {
                                        anim_denormal += 1;
                                    } else if v.abs() > 1.0e6 {
                                        anim_wild += 1;
                                    }
                                    anim_max = anim_max.max(v.abs());
                                }
                            }
                            match err {
                                Some(e) => println!(
                                    "  ANIMFAIL file {af} node {n} script 0x{script:X}: {e}"
                                ),
                                None => {
                                    anim_ok += 1;
                                    if !j.ended() {
                                        anim_running += 1;
                                    }
                                }
                            }
                            anim_frames += frames as u64;
                        }
                    }
                }
            }
            // Same-file only (RE-086's own scope limit): both the script table
            // and the chain-length source it is walked against have to live in
            // the graph's own file for `resolve_scripts` to make sense of them.
            if let Some((mf, mat)) = l.matanim_joints {
                if mf == l.graph.0 {
                    if let Some(f) = loaded.files.get(mf as usize).and_then(Option::as_ref) {
                        let chain_table = match l.mobjsub_table {
                            Some((tf, at)) if tf == l.graph.0 => {
                                ssb_rom::mobj::read_table(f, at, nodes)
                            }
                            _ => None,
                        };
                        let scripts = ssb_rom::matanim::resolve_scripts(f, mat, nodes, |n| {
                            chain_table.as_ref().map_or(0, |t| t.nodes[n].len())
                        });
                        for (node, chain) in scripts.iter().enumerate() {
                            for (m, script) in chain.iter().enumerate() {
                                let Some(script) = *script else { continue };
                                matanim_scripts += 1;
                                let mut j = ssb_rom::matanim::MaterialJoint::start(script, 0.0);
                                let mut max_palette = 0.0f32;
                                let mut frames = 0u32;
                                let mut err = None;
                                loop {
                                    if let Err(e) = j.tick(&f.data, 1.0) {
                                        err = Some(e);
                                        break;
                                    }
                                    frames += 1;
                                    if let Some(v) =
                                        j.track_value(ssb_rom::matanim::TRACK_PALETTE_ID)
                                    {
                                        max_palette = max_palette.max(v);
                                    }
                                    if j.ended() || j.looped() || frames >= ANIM_REPLAY_FRAMES {
                                        break;
                                    }
                                }
                                match err {
                                    Some(e) => {
                                        matanim_fail += 1;
                                        println!(
                                            "  MATANIMFAIL file {mf} script 0x{script:X}: {e}"
                                        );
                                    }
                                    None => {
                                        for (i, count) in matanim_categories.iter_mut().enumerate()
                                        {
                                            if j.track_value(i).is_some() {
                                                *count += 1;
                                            }
                                        }
                                        // Colour-window tracks (PrimColor..Light2Color):
                                        // listed individually, they are rare enough to
                                        // trace one by one.
                                        let colour: Vec<&str> = (ssb_rom::matanim::TICK_EXT_START
                                            ..ssb_rom::matanim::TICK_TRACK_COUNT)
                                            .filter(|&i| j.track_value(i).is_some())
                                            .map(|i| MATANIM_TRACK_NAMES[i])
                                            .collect();
                                        if !colour.is_empty() {
                                            let sub_at = chain_table
                                                .as_ref()
                                                .and_then(|t| t.nodes[node].get(m))
                                                .map_or(0, |m| m.at);
                                            colour_scripts.push(format!(
                                                "file {mf} graph 0x{:X} node {node} MObj {m} MObjSub 0x{sub_at:X} script 0x{script:X}: {}",
                                                l.graph.1,
                                                colour.join(" ")
                                            ));
                                        }
                                        let entries = max_palette.round() as u32 + 1;
                                        if j.track_is_stepped(ssb_rom::matanim::TRACK_PALETTE_ID)
                                            && entries > 1
                                        {
                                            palette_examples.push((mf, l.graph.1, script, entries));
                                            // RE-090: read the real array using
                                            // this computed bound, rather than
                                            // trust that a plausible-looking
                                            // number is a correct one.
                                            let sub_at = chain_table
                                                .as_ref()
                                                .and_then(|t| t.nodes[node].get(m))
                                                .map(|m| m.at);
                                            if let Some(sub_at) = sub_at {
                                                match ssb_rom::mobj::read_palettes(
                                                    f,
                                                    sub_at,
                                                    entries as usize,
                                                ) {
                                                    Some(ptrs) => {
                                                        palette_reads_ok += 1;
                                                        let unique: BTreeSet<_> = ptrs
                                                            .iter()
                                                            .map(|p| (p.file, p.offset))
                                                            .collect();
                                                        if unique.len() != ptrs.len() {
                                                            palette_reads_dup += 1;
                                                        }
                                                    }
                                                    None => {
                                                        palette_reads_fail += 1;
                                                        println!(
                                                            "  PALETTEFAIL file {mf} MObjSub 0x{sub_at:X}: computed {entries} entries but the array does not resolve that many"
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let anim = match (l.anim_joints, l.matanim_joints) {
                (None, None) => String::new(),
                (a, m) => format!(
                    "  [joints {} matanim {}]",
                    a.map_or("-".into(), |(f, at)| format!("{f}@0x{at:X}")),
                    m.map_or("-".into(), |(f, at)| format!("{f}@0x{at:X}")),
                ),
            };
            println!(
                "  layer {}  graph file {} @ 0x{:X} ({nodes} nodes)  {table}{anim}",
                l.index, l.graph.0, l.graph.1
            );
        }
        if let Some((f, at)) = s.map_geometry {
            let decoded = loaded
                .files
                .get(f as usize)
                .and_then(Option::as_ref)
                .and_then(|cf| ssb_rom::collision::read(cf, at));
            match decoded {
                Some(map) => {
                    let n = |k| map.lines_of(k).count();
                    use ssb_rom::collision::LineKind::*;
                    println!(
                        "  collision  file {f} @ 0x{at:X}  {} lines (floor {}, ceiling {}, walls {}), {} map objects",
                        map.lines.len(), n(Floor), n(Ceiling), n(RightWall) + n(LeftWall),
                        map.map_objects.len()
                    );
                    if verbose {
                        for l in &map.lines {
                            let pts: Vec<String> = l
                                .points
                                .iter()
                                .map(|p| format!("({},{})", p.pos[0], p.pos[1]))
                                .collect();
                            println!(
                                "    line {:3} {:?} yak {} {:11}  {}",
                                l.id,
                                l.kind,
                                l.yakumono,
                                surface_flags(l.points[0].flags),
                                pts.join(" ")
                            );
                        }
                        for o in &map.map_objects {
                            println!(
                                "    object kind {:2} at ({},{})",
                                o.kind, o.pos[0], o.pos[1]
                            );
                        }
                    }
                    collision_ok += 1;
                }
                None => {
                    println!("  collision  file {f} @ 0x{at:X}  DOES NOT DECODE");
                    collision_bad += 1;
                }
            }
        }
        if let Some((f, at)) = s.map_nodes {
            println!("  map nodes  file {f} @ 0x{at:X}");
        }
    }
    println!(
        "\n{} stage headers, {layers} render layers ({with_table} with a material table)",
        loaded.stages.len()
    );
    // Replaying every stage joint script is the check that the 32-bit decoder
    // is in step: a desynchronised stream hits an opcode that is not a
    // command long before 600 frames are up (RE-050).
    // The load-bearing number is `still running`, not `replayed`. Ambient stage
    // animation loops forever, so every script should still be going after the
    // frame budget. A stream that desynchronises by one word runs into an
    // `End` early, and the count drops — which is how the `SetInterp` word was
    // caught, since a one-word slip leaves the values plausible and produces no
    // failure at all (RE-050).
    println!(
        "stage joint animations replayed: {anim_ok} script(s), {anim_frames} frame(s), 0 failures"
    );
    println!("  still running after {ANIM_REPLAY_FRAMES} frames: {anim_running}/{anim_ok}");
    let anim_bytes: usize = anim_src_files
        .iter()
        .filter_map(|f| loaded.files.get(*f as usize).and_then(Option::as_ref))
        .map(|f| f.data.len())
        .sum();
    println!(
        "  scripts live in {} file(s), {:.0} KiB if copied whole",
        anim_src_files.len(),
        anim_bytes as f64 / 1024.0
    );
    println!(
        "  pose values: {anim_denormal} denormal, {anim_wild} non-finite or absurd, largest {anim_max:.1}"
    );
    println!("collision maps decoded: {collision_ok}, failed: {collision_bad}");

    // RE-089: cross-checks against RE-086's independently-produced census
    // (172 scripts, 122 PaletteID / 71%) using the shipped `MaterialJoint`
    // engine end-to-end for the first time, and finds the true `palettes[]`
    // bound RE-088 showed cannot come from the ROM's own struct layout.
    println!(
        "stage material animations replayed: {matanim_scripts} script(s), {matanim_fail} failure(s)"
    );
    for (i, name) in MATANIM_TRACK_NAMES.iter().enumerate() {
        if matanim_categories[i] > 0 {
            println!(
                "  {name:<17} {:4} script(s) ({:.0}%)",
                matanim_categories[i],
                100.0 * matanim_categories[i] as f64 / matanim_scripts.max(1) as f64
            );
        }
    }
    println!(
        "  {} script(s) set a colour-window track:",
        colour_scripts.len()
    );
    for line in &colour_scripts {
        println!("    {line}");
    }
    if !palette_examples.is_empty() {
        println!(
            "  {} PaletteID script(s) cycle through more than one palette:",
            palette_examples.len()
        );
        for (file, graph, script, entries) in &palette_examples {
            println!("    file {file} graph 0x{graph:X} script 0x{script:X}: {entries} entries");
        }
        println!(
            "  read_palettes against that bound: {palette_reads_ok} ok, {palette_reads_fail} failed, {palette_reads_dup} with a duplicate entry"
        );
    }

    if let Some(pack_path) = pack_path {
        verify_stage_anims_against_pack(&loaded, &pack_path)?;
    }
    Ok(())
}

/// Reports the `MObj` material tables and cross-checks them.
///
/// Two independent checks, because the pairing is the part worth doubting:
///
/// * Every node's chain length must equal what its display lists ask for, from
///   the segment-`0x0E` entries they call. Nothing in the table says this, so a
///   table paired with the wrong graph would show up here immediately.
/// * With `--expect`, every `MObjSub` offset we resolve must be one the decomp
///   typed by hand (`tools/mobjsub-ground-truth.py`), whose build byte-compares
///   against the ROM.
fn mobj(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::{mobj, scene};

    let mut only_file: Option<u32> = None;
    let mut expect: Option<PathBuf> = None;
    let mut expect_tables: Option<PathBuf> = None;
    let mut search = false;
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            "--expect" => expect = it.next().map(PathBuf::from),
            "--expect-tables" => expect_tables = it.next().map(PathBuf::from),
            "--search" => search = true,
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let Loaded {
        files,
        graphs,
        tables,
        stages,
    } = load_all(&archive);

    let known: Option<BTreeMap<u32, BTreeSet<u32>>> = expect
        .as_deref()
        .map(fs::read_to_string)
        .transpose()?
        .map(|text| {
            let mut map: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let mut cols = line.split('\t');
                if let (Some(f), Some(at)) = (cols.next(), cols.next()) {
                    if let (Ok(f), Ok(at)) = (f.parse(), at.parse()) {
                        map.entry(f).or_default().insert(at);
                    }
                }
            }
            map
        });

    let (mut paired, mut unpaired, mut unreadable) = (0usize, 0usize, 0usize);
    let mut unpaired_graphs: Vec<(u32, u32, usize)> = Vec::new();
    let (mut agree, mut disagree, mut unfollowable) = (0usize, 0usize, 0usize);
    let (mut materials, mut palettes) = (0usize, 0usize);
    let (mut in_decomp, mut not_in_decomp) = (0usize, 0usize);
    // Every `MObjSub` reached, by file and offset, for the `unk10` census.
    let mut subs: BTreeSet<(u32, u32)> = BTreeSet::new();

    for file in files.iter().flatten() {
        if only_file.is_some_and(|f| f != file.id) {
            continue;
        }
        let resolver = scene::DlResolver::new(file);
        for g in graphs.get(&file.id).into_iter().flatten() {
            let Some(offset) = tables.table_for(file.id, g.offset) else {
                if g.nodes.iter().any(|n| {
                    n.desc
                        .dl
                        .is_some_and(|at| mobj_demand(file, &resolver, at) > 0)
                }) {
                    unpaired += 1;
                    // Naming these matters: an unpaired graph draws its nodes
                    // with whatever texture the display list left set, which is
                    // the `G_SETTIMG(0)` the `MObj` was meant to overwrite. The
                    // original pairs them in code, so this list is the ceiling
                    // on what the archive alone can recover (RE-046).
                    unpaired_graphs.push((file.id, g.offset, g.nodes.len()));
                }
                continue;
            };
            // `PartTables::scan` already required this to parse.
            let Some(table) = mobj::read_table(file, offset, g.nodes.len()) else {
                unreadable += 1;
                continue;
            };
            paired += 1;
            for (i, node) in g.nodes.iter().enumerate() {
                let want = node
                    .desc
                    .dl
                    .map_or(0, |at| mobj_demand(file, &resolver, at));
                let have = table.nodes[i].len();
                match () {
                    _ if want == have => {
                        if want > 0 {
                            agree += 1
                        }
                    }
                    // A chain that lives in another archive file reads back
                    // empty; that is a known gap, not a mismatch.
                    _ if have == 0 => unfollowable += 1,
                    _ => {
                        disagree += 1;
                        println!(
                            "  DIFF file {} graph 0x{:X} node {i}: lists call {want} MObj(s), chain has {have}",
                            file.id, g.offset
                        );
                    }
                }
                for m in &table.nodes[i] {
                    materials += 1;
                    subs.insert((file.id, m.at));
                    palettes += usize::from(m.palette.is_some());
                    // A material with no palette is not necessarily wrong —
                    // plenty supply only a primitive colour — but a CI texture
                    // whose TLUT never loads traces back to one, so the chain
                    // it sits in is worth seeing beside it.
                    if only_file.is_some() && m.palette.is_none() {
                        println!(
                            "  NOPAL file {} node {i} MObjSub 0x{:X} (chain {}, demand {want})",
                            file.id,
                            m.at,
                            table.nodes[i].len(),
                        );
                    }
                    if let Some(known) = &known {
                        match known.get(&file.id) {
                            Some(set) if set.contains(&m.at) => in_decomp += 1,
                            Some(_) => {
                                not_in_decomp += 1;
                                println!(
                                    "  GT   file {} MObjSub 0x{:X}: no decomp symbol is placed here",
                                    file.id, m.at
                                );
                            }
                            None => {}
                        }
                    }
                }
            }
        }
    }

    println!(
        "stage headers (MPGroundData): {}\npairings from FTCommonPart and MPGroundDesc: {}",
        stages.len(),
        tables.len()
    );
    println!("graphs paired with a table: {paired} (unreadable {unreadable}, wanting one but unnamed {unpaired})");
    println!("nodes where chain length == display-list demand: {agree}, mismatched: {disagree}");
    println!("  chains in another archive file, not followed: {unfollowable}");
    println!("materials: {materials} ({palettes} carrying a palette)");
    // `gcDrawMObjForDObj` reads `unk10` only under the tile and
    // `gSPTexture` flags (`0x20 | 0x40 | MOBJ_FLAG_TEXTURE`, with
    // `MOBJ_FLAG_NONE` read as `0xA1`), so a value other than 0 or 2 matters
    // only beside one of them (RE-327). Counted over the table chains and
    // every `--expect` offset, from the raw bytes.
    for (f, set) in known.iter().flatten() {
        subs.extend(set.iter().map(|&at| (*f, at)));
    }
    let mut unk10: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
    for &(f, at) in &subs {
        let Some(data) = files
            .get(f as usize)
            .and_then(Option::as_ref)
            .map(|x| &x.data)
        else {
            continue;
        };
        let word = |o: usize| {
            data.get(o..o + 4)
                .map(|b| i32::from_be_bytes(b.try_into().unwrap()))
        };
        let half = |o: usize| {
            data.get(o..o + 2)
                .map(|b| u16::from_be_bytes(b.try_into().unwrap()))
        };
        let (Some(v), Some(flags)) = (word(at as usize + 0x10), half(at as usize + 0x30)) else {
            continue;
        };
        let flags = if flags == 0 { 0xA1 } else { flags };
        let e = unk10.entry(v).or_default();
        e.0 += 1;
        if flags & 0xE0 != 0 {
            e.1 += 1;
        } else if v == 1 {
            println!("  UNK10 file {f} MObjSub 0x{at:X}: 1, flags 0x{flags:04X}");
        }
    }
    for (v, (n, windowed)) in &unk10 {
        println!("MObjSub.unk10 == {v}: {n} ({windowed} with a tile or gSPTexture flag)");
    }
    if !unpaired_graphs.is_empty() && !search {
        println!("\ngraphs that call the graphics heap but no record names:");
        for (file, offset, nodes) in unpaired_graphs.iter().take(12) {
            println!("  file {file:<5} graph 0x{offset:<6X} {nodes} node(s)");
        }
        if unpaired_graphs.len() > 12 {
            println!("  ... and {} more", unpaired_graphs.len() - 12);
        }
    }
    if search {
        search_unpaired_tables(&files, &graphs, &unpaired_graphs, expect_tables.as_deref())?;
    }
    if known.is_some() {
        // The generator can only place a struct the decomp gives an offset
        // for, by comment or by symbol name; a few are hand-named with
        // neither. A miss here means "unlocatable", not "contradicted".
        println!(
            "MObjSubs at an offset the decomp places: {in_decomp}, elsewhere: {not_in_decomp}"
        );
    }
    Ok(())
}

/// Searches for the material table of every graph no record names, and scores
/// the result against the decomp's own declarations.
///
/// The point is the score, not the search. A search that returns one candidate
/// for a graph has identified it; a search that returns three has identified
/// nothing, and the difference has to be measured before any of it is trusted
/// (RE-046). `--expect-tables` supplies the answer key, generated by
/// `tools/mobjtable-ground-truth.py` from the decomp's `MObjSub **name[]`
/// declarations.
fn search_unpaired_tables(
    files: &[Option<ssb_rom::archive::File>],
    graphs: &BTreeMap<u32, Vec<ssb_rom::scene::SceneGraph>>,
    unpaired: &[(u32, u32, usize)],
    expect_tables: Option<&Path>,
) -> Res {
    let key: Option<BTreeMap<u32, BTreeSet<u32>>> = expect_tables
        .map(fs::read_to_string)
        .transpose()?
        .map(|text| {
            let mut map: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let mut cols = line.split('\t');
                if let (Some(f), Some(at)) = (cols.next(), cols.next()) {
                    if let (Ok(f), Ok(at)) = (f.parse(), at.parse()) {
                        map.entry(f).or_default().insert(at);
                    }
                }
            }
            map
        });

    let (mut unique, mut ambiguous, mut none) = (0usize, 0usize, 0usize);
    let (mut confirmed, mut contradicted, mut unkeyed) = (0usize, 0usize, 0usize);
    let mut found: Vec<(u32, u32, u32)> = Vec::new();

    for &(file_id, graph_offset, _) in unpaired {
        let Some(file) = files.get(file_id as usize).and_then(Option::as_ref) else {
            continue;
        };
        let Some(graph) = graphs
            .get(&file_id)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph_offset))
        else {
            continue;
        };
        let resolver = ssb_rom::scene::DlResolver::new(file);
        let demand: Vec<usize> = graph
            .nodes
            .iter()
            .map(|n| n.desc.dl.map_or(0, |at| mobj_demand(file, &resolver, at)))
            .collect();

        let hits = ssb_rom::mobj::search_tables(file, &demand);
        match hits.len() {
            0 => none += 1,
            1 => {
                unique += 1;
                let at = hits[0];
                found.push((file_id, graph_offset, at));
                match key.as_ref().and_then(|k| k.get(&file_id)) {
                    Some(set) if set.contains(&at) => confirmed += 1,
                    Some(set) => {
                        contradicted += 1;
                        let near = set
                            .iter()
                            .map(|&k| format!("0x{k:X}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!(
                            "  WRONG file {file_id} graph 0x{graph_offset:X}: search says \
                             0x{at:X}, decomp declares [{near}]"
                        );
                    }
                    None => unkeyed += 1,
                }
            }
            n => {
                ambiguous += 1;
                let all = hits
                    .iter()
                    .map(|&h| format!("0x{h:X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("  AMBIG file {file_id} graph 0x{graph_offset:X}: {n} candidates [{all}]");
            }
        }
    }

    println!(
        "\nmaterial-table search over {} unnamed graph(s):",
        unpaired.len()
    );
    println!("  exactly one candidate   {unique}");
    println!("  several candidates      {ambiguous}");
    println!("  none                    {none}");
    if key.is_some() {
        println!("  of the unique ones, against the decomp's declarations:");
        println!("    confirmed             {confirmed}");
        println!("    contradicted          {contradicted}");
        println!("    file has no key entry {unkeyed}");
    }
    Ok(())
}

/// How many `MObj`s the lists hanging off a node's `dl` slot call for.
fn mobj_demand(
    file: &ssb_rom::archive::File,
    resolver: &ssb_rom::scene::DlResolver,
    at: u32,
) -> usize {
    resolver
        .lists(at)
        .iter()
        .map(|&l| {
            file.data
                .get(l as usize..)
                .and_then(|d| ssb_rom::dl::decode_list(d).ok())
                .map_or(0, |cmds| ssb_rom::mobj::demand(&cmds, &file.data))
        })
        .max()
        .unwrap_or(0)
}

/// Writes decoded textures out as PPM so they can be eyeballed.
///
/// The point is to separate two very different failures: a texture that
/// converts correctly on the host but renders as noise on device means the
/// bug is in swizzling, format or upload; one that is already noise here means
/// the bug is upstream, in the offsets or palettes the display list gave us.
fn texdump(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::texture;

    let mut only_file: Option<u32> = None;
    let mut count = 12usize;
    let mut out_dir = PathBuf::from("assets/generated/texdump");

    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            "--count" => count = it.next().ok_or("--count needs a number")?.parse()?,
            "--out" => out_dir = it.next().ok_or("--out needs a dir")?.into(),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    fs::create_dir_all(&out_dir)?;
    let loaded = load_all(&archive);

    let mut written = 0usize;
    let mut seen: std::collections::BTreeSet<(u32, u32)> = std::collections::BTreeSet::new();

    for id in 0..archive.len() as u32 {
        if written >= count {
            break;
        }
        if only_file.is_some_and(|f| f != id) {
            continue;
        }
        let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
            continue;
        };

        // Convert the way the packer does, not with `convert`. A fighter's
        // palette comes from its `MObj` chain, so a standalone conversion dumps
        // its textures unpalettised -- which would make this tool report a
        // problem the pack does not have, and hide one it does.
        for m in file_meshes(&loaded, file) {
            if written >= count {
                break;
            }
            for prim in &m.primitives {
                if written >= count {
                    break;
                }
                let Some(t) = prim.material.texture else {
                    continue;
                };
                // A stage's texels live in a different archive file, so the
                // bytes have to come from wherever the relocation led — dumping
                // them out of the drawing file gives noise that looks exactly
                // like a broken decoder (RE-037, RE-047).
                let texels = Texels {
                    home: file,
                    all: &loaded.files,
                };
                let home = t.data_file.map_or(id, u32::from);
                if !seen.insert((home, t.data_offset)) {
                    continue;
                }
                if (t.data_offset >> 24) != 0 || t.data_offset == 0 {
                    continue;
                }

                let need = texture::data_len(t.width as u32, t.height as u32, t.size);
                let Some(src) = texels
                    .bytes(t.data_file)
                    .and_then(|d| d.get(t.data_offset as usize..t.data_offset as usize + need))
                else {
                    continue;
                };
                let tlut: Vec<u16> = match t.palette_offset {
                    Some(off) => {
                        let (off, n) = palette_bank_offset(off, t.palette_entries, t.palette);
                        texels
                            .bytes(t.palette_file)
                            .and_then(|d| d.get(off as usize..off as usize + n * 2))
                            .map(texture::parse_tlut)
                            .unwrap_or_default()
                    }
                    None => Vec::new(),
                };

                let Ok(img) = texture::decode(
                    src,
                    t.width as u32,
                    t.height as u32,
                    t.format,
                    t.size,
                    (!tlut.is_empty()).then_some(tlut.as_slice()),
                ) else {
                    continue;
                };

                // PPM: trivially writable without a PNG dependency, and
                // ImageMagick can convert it for viewing.
                let mut ppm = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
                for px in img.pixels.as_chunks::<4>().0 {
                    ppm.extend_from_slice(&px[..3]);
                }
                // Named by the file the *texels* are in, not the one drawing
                // them, so two stages sharing a texture do not look like two.
                let name = format!(
                    "f{home}_o{:X}_{:?}{}_{}x{}.ppm",
                    t.data_offset,
                    t.format,
                    t.size.bits(),
                    t.width,
                    t.height
                );
                fs::write(out_dir.join(&name), &ppm)?;
                println!("  {name}  palette {} entries", tlut.len());
                written += 1;
            }
        }
    }

    println!("wrote {written} textures to {}", out_dir.display());
    Ok(())
}

fn parse_id(s: &str) -> Result<u32, Box<dyn std::error::Error>> {
    s.strip_prefix("0x")
        .map(|h| u32::from_str_radix(h, 16))
        .unwrap_or_else(|| s.parse())
        .map_err(|e| format!("bad file id {s:?}: {e}").into())
}

fn dump(path: &Path, id: &str) -> Res {
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    let id = parse_id(id)?;

    let entry = archive.entry(id).ok_or("file id out of range")?;
    let file = archive.load(id)?;

    println!("file {id}");
    println!("  compressed     {}", entry.compressed);
    println!(
        "  rom offset     0x{:08X}",
        archive.data_base() + entry.data_offset as usize
    );
    println!("  packed         {} bytes", entry.rom_size());
    println!("  unpacked       {} bytes", file.data.len());
    println!("  intern relocs  {}", file.intern_relocs.len());
    println!("  extern relocs  {}", file.extern_relocs.len());

    let deps: BTreeMap<u16, usize> =
        file.extern_relocs
            .iter()
            .fold(BTreeMap::new(), |mut acc, r| {
                *acc.entry(r.target_file).or_default() += 1;
                acc
            });
    if !deps.is_empty() {
        println!("  depends on:");
        for (target, count) in deps {
            println!("    file {target:<5} ({count} pointer(s))");
        }
    }

    let out = PathBuf::from("assets/generated/dump").join(format!("{id}.bin"));
    fs::create_dir_all(out.parent().unwrap())?;
    fs::write(&out, &file.data)?;
    println!("  written to     {}", out.display());
    Ok(())
}

/// Walks a file's display lists and reports the texture loads it performs.
///
/// This is the reconnaissance step for the rendering pipeline: before writing
/// a converter we need to know which `(format, size)` combinations Smash
/// actually uses, rather than supporting all of them speculatively.
/// Extracts and packs every texture the game's display lists actually bind,
/// then reports the VRAM budget.
///
/// Textures are found via `mesh::convert`, which resolves each primitive's
/// `TextureRef` from the RDP state in force at draw time -- so this covers the
/// textures the game really uses, not every image-shaped blob in the archive.
fn textures(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture;

    let mut only_file: Option<u32> = None;
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let loaded = load_all(&archive);

    // Deduplicate: the same texture is bound by many primitives.
    let mut seen: std::collections::BTreeSet<(u32, u32, u16, u16)> =
        std::collections::BTreeSet::new();
    let mut by_format: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // count, bytes
    let mut packed_ok = 0usize;
    let mut decode_failed = 0usize;
    let mut total_psp = 0usize;
    let mut total_naive = 0usize;
    let mut swizzled = 0usize;
    let mut largest: Vec<(usize, u32, u32, String)> = Vec::new();
    let mut why: BTreeMap<String, usize> = BTreeMap::new();
    // Per-file tallies. A failure rate spread thinly across the archive is a
    // different problem from one file losing every texture it binds, and only
    // the second explains a stage that draws white.
    let mut per_file: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    // A `G_SETTIMG(0)` that nothing overwrote. Attributing these to a file is
    // what turns "54 failures" into a list of graphs whose material table was
    // never named -- the difference between a decoder bug and a missing
    // pairing (RE-046).
    let mut nulls_by_file: BTreeMap<u32, Vec<(u16, u16)>> = BTreeMap::new();
    // Every failure reason, per file. Which files a class is concentrated in
    // is what says whether it is one decoder gap or a scatter of unrelated
    // ones -- the null pointers turned out to be 54 files with one apiece.
    let mut fails_by_file: BTreeMap<(String, u32), usize> = BTreeMap::new();

    // Same conversion the packer runs, so these counts describe the pack. On
    // the standalone path a fighter's textures come out palette-less, and this
    // reported 142 failures that the shipped pack does not have.
    for file in loaded.files.iter().flatten() {
        if only_file.is_some_and(|f| f != file.id) {
            continue;
        }
        for m in file_meshes(&loaded, file) {
            for prim in &m.primitives {
                let Some(t) = prim.material.texture else {
                    continue;
                };
                // Key on where the *texels* are. Keying on the drawing file
                // made every unresolved cross-file texture collapse into one
                // entry, since they all had offset zero, and hid the scale of
                // RE-037 completely.
                let home = t.data_file.map_or(file.id, u32::from);
                // An unresolved texture has offset zero, so every one in a file
                // hashes to the same key. Keying those on the *size* as well
                // keeps them apart -- otherwise a file with thirty unresolved
                // bindings reports one, and the archive-wide total counts files
                // rather than textures (RE-046).
                let key = (home, t.data_offset, t.width, t.height);
                if !seen.insert(key) {
                    continue;
                }
                if only_file.is_some() {
                    // UVs are S10.5 fixed point: 32 units per texel. How many
                    // times a texture repeats across a surface is the span in
                    // texels divided by its size, and an over-tiled surface is
                    // the difference between foliage and green noise.
                    let (mut u0, mut u1, mut v0, mut v1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
                    for i in &prim.indices {
                        if let Some(v) = m.vertices.get(*i as usize) {
                            u0 = u0.min(v.uv[0] as i32);
                            u1 = u1.max(v.uv[0] as i32);
                            v0 = v0.min(v.uv[1] as i32);
                            v1 = v1.max(v.uv[1] as i32);
                        }
                    }
                    let (su, sv) = ((u1 - u0) as f32 / 32.0, (v1 - v0) as f32 / 32.0);
                    println!(
                        "  bind {}x{} {:?}/{:?} <- file {:?} +0x{:X} tlut {:?}  uv span {:.1}x{:.1} texels = {:.2}x{:.2} repeats",
                        t.width,
                        t.height,
                        t.format,
                        t.size,
                        t.data_file,
                        t.data_offset,
                        t.palette_offset,
                        su,
                        sv,
                        su / t.width.max(1) as f32,
                        sv / t.height.max(1) as f32,
                    );
                }
                let texels = match t.data_file {
                    None => &file.data[..],
                    Some(id) => match loaded.files.get(id as usize).and_then(Option::as_ref) {
                        Some(f) => &f.data[..],
                        None => {
                            *why.entry("cross-file, target file did not load".into())
                                .or_default() += 1;
                            *fails_by_file
                                .entry(("cross-file, target file did not load".into(), file.id))
                                .or_default() += 1;
                            decode_failed += 1;
                            continue;
                        }
                    },
                };

                let psm = psp::choose_psm(t.format, t.size);
                let _ = &psm;
                let need = texture::data_len(t.width as u32, t.height as u32, t.size);
                let at = t.data_offset as usize;

                // Diagnose *why* a texture cannot be read, rather than lumping
                // every failure together.
                per_file.entry(file.id).or_default().0 += 1;
                let segment = (t.data_offset >> 24) as u8;
                if segment != 0 {
                    *why.entry(format!("segmented addr (seg 0x{segment:02X})"))
                        .or_default() += 1;
                    *fails_by_file
                        .entry((format!("segmented addr (seg 0x{segment:02X})"), file.id))
                        .or_default() += 1;
                    decode_failed += 1;
                    continue;
                }
                if t.data_offset == 0 && t.data_file.is_none() {
                    *why.entry("null pointer, nothing resolved it".into())
                        .or_default() += 1;
                    *fails_by_file
                        .entry(("null pointer, nothing resolved it".into(), file.id))
                        .or_default() += 1;
                    nulls_by_file
                        .entry(file.id)
                        .or_default()
                        .push((t.width, t.height));
                    decode_failed += 1;
                    continue;
                }
                let Some(src) = texels.get(at..at + need) else {
                    *why.entry("offset past end of file".into()).or_default() += 1;
                    *fails_by_file
                        .entry(("offset past end of file".into(), file.id))
                        .or_default() += 1;
                    decode_failed += 1;
                    continue;
                };
                // Only a *CI* texture needs a TLUT out of the ROM. Intensity
                // formats also map to a PSP CLUT format, but they generate
                // their ramp, so counting them here listed 29 warnings against
                // textures that convert perfectly.
                if t.format == texture::Format::Ci && t.palette_offset.is_none() {
                    *why.entry("CI texture, no TLUT recorded".into())
                        .or_default() += 1;
                    *fails_by_file
                        .entry(("CI texture, no TLUT recorded".into(), file.id))
                        .or_default() += 1;
                }

                // Palette, if this is a CLUT format.
                let tlut: Vec<u16> = match t.palette_offset {
                    Some(off) => {
                        let (off, entries) = palette_bank_offset(off, t.palette_entries, t.palette);
                        let pal = match t.palette_file {
                            None => Some(&file.data[..]),
                            Some(id) => loaded
                                .files
                                .get(id as usize)
                                .and_then(Option::as_ref)
                                .map(|f| &f.data[..]),
                        };
                        pal.and_then(|p| p.get(off as usize..off as usize + entries * 2))
                            .map(texture::parse_tlut)
                            .unwrap_or_default()
                    }
                    None => Vec::new(),
                };

                // The packer's own conversion, so these counts describe the
                // pack rather than a parallel implementation of it. Keeping a
                // second copy here is how the report came to quote a VRAM
                // figure with no mip levels in it while the pack shipped them
                // (RE-053). The classification above still explains *why*
                // something fails; this decides whether it does.
                let (gate, translucent_blend) = ssb_rom::pack::material_alpha_state(&prim.material);
                let alpha_policy =
                    ssb_rom::filter_compensation::AlphaPolicy::classify(gate, translucent_blend);
                let tex = convert_texture(
                    Texels {
                        home: file,
                        all: &loaded.files,
                    },
                    &t,
                    &filter_coverage::Coverage::default(),
                    true,
                    None,
                    alpha_policy,
                    None,
                    None,
                );
                if tex.is_none() {
                    let reason = match texture::decode(
                        src,
                        t.width as u32,
                        t.height as u32,
                        t.format,
                        t.size,
                        (!tlut.is_empty()).then_some(tlut.as_slice()),
                    ) {
                        Err(e) => format!("decode: {e:?}"),
                        Ok(_) => "conversion declined".into(),
                    };
                    *why.entry(reason.clone()).or_default() += 1;
                    *fails_by_file.entry((reason, file.id)).or_default() += 1;
                }

                match tex {
                    Some(tex) => {
                        packed_ok += 1;
                        per_file.entry(file.id).or_default().1 += 1;
                        if tex.swizzled {
                            swizzled += 1;
                        }
                        let size = tex.vram_size();
                        total_psp += size;
                        // What it would cost expanded to 32-bit RGBA.
                        total_naive += (t.width as usize) * (t.height as usize) * 4;

                        let name = format!("{:?}", tex.format);
                        let e = by_format.entry(name.clone()).or_default();
                        e.0 += 1;
                        e.1 += size;
                        largest.push((size, t.width as u32, t.height as u32, name));
                    }
                    None => decode_failed += 1,
                }
            }
        }
    }

    let kib = |b: usize| b as f64 / 1024.0;

    println!("texture conversion");
    println!("  unique textures bound  {}", seen.len());
    println!("  packed                 {packed_ok}");
    println!("  failed                 {decode_failed}");
    // A note is not a failure. Splitting them is what stops the reasons summing
    // to more than the failure count and inviting the reader to add them up.
    let is_note = |r: &str| r == "CI texture, no TLUT recorded";
    for (reason, n) in why.iter().filter(|(r, _)| !is_note(r)) {
        println!("    {reason:<48} {n:>4}");
    }
    for (reason, n) in why.iter().filter(|(r, _)| is_note(r)) {
        println!("  note: {reason:<45} {n:>4}");
    }
    println!(
        "  swizzled               {swizzled} ({:.0}%)",
        swizzled as f64 / packed_ok.max(1) as f64 * 100.0
    );

    println!("\nby PSP format:");
    for (fmt, (n, bytes)) in &by_format {
        println!("  {fmt:<8} {n:>5} textures  {:>9.1} KiB", kib(*bytes));
    }

    println!("\nVRAM budget");
    println!("  packed (chosen formats)  {:>9.1} KiB", kib(total_psp));
    println!("  naive, all RGBA8888      {:>9.1} KiB", kib(total_naive));
    if total_naive > 0 {
        println!(
            "  saving                   {:>9.1}%",
            100.0 - (total_psp as f64 / total_naive as f64 * 100.0)
        );
    }
    // Framebuffers + depth leave roughly this much of the PSP's 2 MiB VRAM.
    const VRAM_FOR_TEXTURES: usize = 700 * 1024;
    println!(
        "  fits in ~700 KiB texture VRAM? {}",
        if total_psp <= VRAM_FOR_TEXTURES {
            "yes, all at once".into()
        } else {
            format!(
                "no - needs streaming ({:.1}x over)",
                total_psp as f64 / VRAM_FOR_TEXTURES as f64
            )
        }
    );

    let mut worst: Vec<(u32, usize, usize)> = per_file
        .iter()
        .filter(|(_, (bound, packed))| packed < bound)
        .map(|(&id, &(bound, packed))| (id, bound, packed))
        .collect();
    worst.sort_by_key(|(_, bound, packed)| std::cmp::Reverse(bound - packed));
    println!(
        "\nfiles losing textures ({} of {} files that bind any):",
        worst.len(),
        per_file.len()
    );
    for (id, bound, packed) in worst.iter().take(12) {
        println!("  file {id:<5} {packed:>3}/{bound:<3} packed");
    }
    if let Some(id) = only_file {
        match per_file.get(&id) {
            Some((bound, packed)) => println!("\nfile {id}: {packed}/{bound} textures packed"),
            None => println!("\nfile {id}: binds no textures at all"),
        }
    }

    println!("\nwhere each failure class lives:");
    for reason in why.keys() {
        let mut per: Vec<(u32, usize)> = fails_by_file
            .iter()
            .filter(|((r, _), _)| r == reason)
            .map(|((_, f), &n)| (*f, n))
            .collect();
        per.sort_by_key(|&(f, n)| (std::cmp::Reverse(n), f));
        let total: usize = per.iter().map(|&(_, n)| n).sum();
        let head: Vec<String> = per
            .iter()
            .take(6)
            .map(|(f, n)| format!("{f}:{n}"))
            .collect();
        println!(
            "  {reason:<42} {total:>3} in {:>2} file(s)  {}{}",
            per.len(),
            head.join(" "),
            if per.len() > 6 { " ..." } else { "" }
        );
    }

    if !nulls_by_file.is_empty() {
        let total: usize = nulls_by_file.values().map(Vec::len).sum();
        println!(
            "\nunresolved G_SETTIMG(0), by file ({total} across {} files):",
            nulls_by_file.len()
        );
        let mut ranked: Vec<_> = nulls_by_file.iter().collect();
        ranked.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        for (id, v) in ranked.iter().take(12) {
            println!("  file {id:<5} {:>2} texture(s)", v.len());
        }
    }

    largest.sort_by_key(|(s, ..)| std::cmp::Reverse(*s));
    println!("\nlargest textures:");
    for (size, w, h, fmt) in largest.iter().take(8) {
        println!("  {w:>3}x{h:<3} {fmt:<8} {:>8.1} KiB", kib(*size));
    }

    Ok(())
}

fn extract(path: &Path, opts: &[&str]) -> Res {
    let mut out_dir = PathBuf::from("assets/generated");
    let mut limit = usize::MAX;

    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--out" => out_dir = it.next().ok_or("--out needs a directory")?.into(),
            "--limit" => {
                limit = it
                    .next()
                    .ok_or("--limit needs a count")?
                    .parse()
                    .map_err(|e| format!("bad --limit: {e}"))?
            }
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let files_dir = out_dir.join("files");
    fs::create_dir_all(&files_dir)?;

    let count = archive.len().min(limit);
    let mut manifest = String::from(
        "# relocData manifest\n\
         # Generated by romtool. Do not commit -- derived from a copyrighted ROM.\n\
         # id\tsize\tcompressed\tintern\textern\tdeps\n",
    );
    let mut failures = Vec::new();
    let mut total = 0usize;

    for id in 0..count as u32 {
        match archive.load(id) {
            Ok(file) => {
                fs::write(files_dir.join(format!("{id:04}.bin")), &file.data)?;
                total += file.data.len();

                let mut deps: Vec<u16> = file.extern_relocs.iter().map(|r| r.target_file).collect();
                deps.sort_unstable();
                deps.dedup();
                let deps = deps
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(",");

                manifest.push_str(&format!(
                    "{id}\t{}\t{}\t{}\t{}\t{}\n",
                    file.data.len(),
                    archive.entry(id).is_some_and(|e| e.compressed),
                    file.intern_relocs.len(),
                    file.extern_relocs.len(),
                    deps
                ));
            }
            Err(e) => failures.push((id, e)),
        }
    }

    fs::write(out_dir.join("manifest.tsv"), manifest)?;

    println!(
        "extracted {}/{count} files to {}",
        count - failures.len(),
        files_dir.display()
    );
    println!("total unpacked: {:.2} MiB", total as f64 / (1 << 20) as f64);

    if !failures.is_empty() {
        eprintln!("\n{} file(s) failed:", failures.len());
        for (id, e) in failures.iter().take(20) {
            eprintln!("  file {id}: {e}");
        }
        if failures.len() > 20 {
            eprintln!("  ... and {} more", failures.len() - 20);
        }
        return Err(format!("{} file(s) failed to extract", failures.len()).into());
    }
    Ok(())
}

/// Reads every fighter's animation lengths out of the ROM.
///
/// The decode is self-checking: each animation file holds one script per model
/// joint, and `decode_length` requires all of them to agree. `--verify` adds
/// the second, independent reading — the lengths `tools/gen-anim-table.py`
/// computed from the decompilation's hand-written C sources.
/// Inspect selected figatree file durations without adding temporary
/// fighter slots to the pack (useful when porting character statuses).
fn anim_length(path: &Path, ids: &[&str]) -> Res {
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    for raw in ids {
        let id: u32 = raw.parse()?;
        let file = archive.load(id)?;
        let length = ssb_rom::anim::decode_length(id, &file)?;
        println!("{id}: {length:?}");
    }
    Ok(())
}

fn anims(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::anim;

    let mut verify = false;
    for o in opts {
        match *o {
            "--verify" | "-v" => verify = true,
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    print!("{:<10}", "fighter");
    for name in anim::SLOT_NAMES {
        print!(" {name:>9}");
    }
    println!();

    let mut looping = Vec::new();
    let mut decoded = Vec::new();
    for entry in anim::FIGHTER_ANIMS {
        let lengths = anim::decode_fighter(entry, &archive)?;
        print!("{:<10}", entry.name);
        for (slot, &frames) in lengths.frames.iter().enumerate() {
            if frames == 0 {
                // Only the first seven slots are supposed to end; the rest
                // loop until interrupted, and some are absent outright where
                // the character lacks the move (RE-035, RE-041).
                if slot < anim::TIMED_SLOTS {
                    looping.push((entry.name, ssb_rom::anim::SLOT_NAMES[slot]));
                }
                print!(" {:>9}", if entry.files[slot] == 0 { "-" } else { "loops" });
            } else {
                print!(" {frames:>9}");
            }
        }
        println!();
        decoded.push(lengths);
    }

    println!();
    println!("{} fighters decoded, all joints agreed", decoded.len());
    // Master Hand's whole common status table points at one looping idle; it
    // never walks or dashes. Anyone else looping means a wrong file id.
    let unexpected: Vec<_> = looping
        .iter()
        .filter(|(name, _)| *name != "Boss")
        .map(|(name, slot)| format!("{name}.{slot}"))
        .collect();
    if !unexpected.is_empty() {
        return Err(format!("looping animation for {}", unexpected.join(", ")).into());
    }

    if !verify {
        println!("(pass --verify to cross-check against the decompilation)");
        return Ok(());
    }

    let mut mismatches = Vec::new();
    let mut fields = 0usize;
    for (lengths, want) in decoded.iter().zip(anim::EXPECTED_FRAMES.iter()) {
        for (slot, (&got, &w)) in lengths.frames.iter().zip(want.iter()).enumerate() {
            // Every slot the decompilation gives a finite length for is
            // compared; a looping or absent slot is 0 on both sides, and the
            // first seven must never be 0 (checked above).
            if slot >= anim::TIMED_SLOTS && got == 0 && w == 0 {
                continue;
            }
            fields += 1;
            if got != w {
                mismatches.push(format!(
                    "{}.{}: rom {got} vs decomp {w}",
                    lengths.name,
                    anim::SLOT_NAMES[slot]
                ));
            }
        }
    }

    println!("verified    {fields} lengths against the decompilation");
    if mismatches.is_empty() {
        println!("            all agree");
        Ok(())
    } else {
        for m in mismatches.iter().take(20) {
            println!("  !! {m}");
        }
        Err(format!("{} length(s) disagree", mismatches.len()).into())
    }
}

/// Plays a fighter's animation and reports what it does to the skeleton.
///
/// The two structural claims this checks are the ones the whole animation
/// pipeline rests on. First, that a figatree's joint pointer table lines up
/// one-for-one with the `DObjDesc` array of the fighter's model — which is
/// what `gcAddAnimJointAll` says, walking both in lockstep:
///
/// ```c
/// while (dobj != NULL) {
///     if (*anim_joints != NULL) gcAddDObjAnimJoint(dobj, *anim_joints, frame);
///     anim_joints++;
///     dobj = gcGetTreeDObjNext(dobj);
/// }
/// ```
///
/// Second, that the value scales in `ftAnimGetTargetValue` are right: an
/// animated joint's translation has to land near its rest translation, because
/// a skeleton is not rebuilt from scratch every frame. A wrong divisor there
/// would show up as joints thrown hundreds of units apart.
fn figatree(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::{anim, figatree as fg, fighter};

    let mut want_fighter: Option<String> = None;
    let mut want_slot: Option<String> = None;
    let mut frames = 0usize;
    let mut pack_path: Option<PathBuf> = None;
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--fighter" => want_fighter = it.next().map(|s| s.to_string()),
            "--slot" => want_slot = it.next().map(|s| s.to_string()),
            "--frames" => frames = it.next().ok_or("--frames needs a count")?.parse()?,
            "--pack" => pack_path = Some(PathBuf::from(it.next().ok_or("--pack needs a path")?)),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    let loaded = load_all(&archive);

    // Which graph is a fighter's skeleton comes from the `FTCommonPart`
    // records in its own `*Main` file, not from picking the biggest graph in
    // the archive: Samus has two 33-node graphs and a fits-the-shape search
    // chooses between them at close to chance (RE-027).
    //
    // `setup_parts` then says which of that graph's descriptors actually
    // become joints, which is what a figatree's pointer table is sized to.
    let mut skeletons: BTreeMap<&str, Skeleton> = BTreeMap::new();
    for (i, entry) in fighter::FIGHTER_FILES.iter().enumerate() {
        let name = anim::FIGHTER_ANIMS[i].name;
        let Some(main) = loaded.files[entry.file as usize].as_ref() else {
            continue;
        };
        let mask = fighter::setup_parts(main, *entry).unwrap_or(u64::MAX);
        // The high-detail entry of `FTCommonPartContainer` — the skeleton the
        // game builds a fighter from at full detail.
        let Some(part) = fighter::common_parts(main, *entry)[0] else {
            continue;
        };
        let Some(nodes) = loaded
            .graphs
            .get(&part.model_file)
            .and_then(|gs| gs.iter().find(|g| g.offset == part.graph))
            .map(|g| g.nodes.len())
        else {
            continue;
        };
        skeletons.insert(
            name,
            Skeleton {
                file: part.model_file,
                offset: part.graph,
                nodes,
                mask,
            },
        );
    }

    println!(
        "{:<10} {:>6} {:>6} {:>7}  {:<9} {:>5} {:>6} {:>7}",
        "fighter", "model", "descs", "joints", "slot", "file", "table", "scripts"
    );

    let mut checked = 0usize;
    let mut mismatched = Vec::new();
    let mut worst_drift = 0.0f32;
    for entry in anim::FIGHTER_ANIMS {
        if want_fighter.as_deref().is_some_and(|w| w != entry.name) {
            continue;
        }
        let skeleton = skeletons.get(entry.name);
        // Descriptor index of each joint, in the order the setup walk creates
        // them. That order is the descriptor array's, so the nth animation
        // script belongs to the nth *set* mask bit.
        let joint_descs: Vec<usize> = skeleton
            .map(|s| (0..s.nodes).filter(|&i| s.mask >> i & 1 != 0).collect())
            .unwrap_or_default();

        for (slot, &id) in entry.files.iter().enumerate() {
            let slot_name = anim::SLOT_NAMES[slot];
            if want_slot.as_deref().is_some_and(|w| w != slot_name) {
                continue;
            }
            if id == 0 {
                continue; // a move this fighter does not have
            }
            let file = archive.load(id as u32)?;
            let table = joint_table(&file.data)
                .ok_or_else(|| format!("{}.{slot_name}: no joint table", entry.name))?;
            let scripts = table.iter().filter(|p| **p != 0).count();
            println!(
                "{:<10} {:>6} {:>6} {:>7}  {:<9} {:>5} {:>6} {:>7}",
                entry.name,
                skeleton.map_or(0, |s| s.file),
                skeleton.map_or(0, |s| s.nodes),
                joint_descs.len(),
                slot_name,
                id,
                table.len(),
                scripts
            );
            checked += 1;
            // The attach walk is bounded by the fighter's DObj tree, so a
            // table may be longer than the fighter has joints and the surplus
            // simply goes unread — which is how the polygon-model variants
            // share the full character's animations. It may never be shorter:
            // that would leave a joint reading a pointer past the array.
            if !joint_descs.is_empty() && table.len() < joint_descs.len() {
                mismatched.push(format!(
                    "{}.{}: {} scripts in the animation, {} joints in the fighter",
                    entry.name,
                    slot_name,
                    table.len(),
                    joint_descs.len()
                ));
            }

            if frames == 0 {
                continue;
            }
            // Play it, and show each joint against the rest pose it starts from.
            let rest = skeleton.and_then(|s| {
                loaded
                    .graphs
                    .get(&s.file)
                    .and_then(|gs| gs.iter().find(|g| g.offset == s.offset))
            });
            for (joint, &start) in table.iter().enumerate() {
                if start == 0 {
                    continue;
                }
                let mut anim = fg::JointAnim::start(start as usize, 0.0);
                let desc = joint_descs
                    .get(joint)
                    .and_then(|&d| rest.and_then(|g| g.nodes.get(d)));
                let mut pose = match desc {
                    Some(n) => fg::JointPose {
                        rotate: n.desc.rotate,
                        translate: n.desc.translate,
                        scale: n.desc.scale,
                    },
                    None => fg::JointPose::default(),
                };
                let rest_t = pose.translate;
                let mut drift: f32 = 0.0;
                for frame in 0..frames {
                    if let Err(e) = anim.tick(&file.data, 1.0, &mut pose) {
                        println!("  joint {joint:>2} script {start:#06x}: frame {frame}: {e}");
                        mismatched.push(format!("{}.{slot_name} joint {joint}: {e}", entry.name));
                        break;
                    }
                    let d = (0..3)
                        .map(|i| (pose.translate[i] - rest_t[i]).abs())
                        .fold(0.0f32, f32::max);
                    drift = drift.max(d);
                }
                worst_drift = worst_drift.max(drift);
                println!(
                    "  joint {joint:>2} -> desc {:>2}  rest t {:>8.2} {:>8.2} {:>8.2}   \
                     played t {:>8.2} {:>8.2} {:>8.2}   r {:>6.2} {:>6.2} {:>6.2}   drift {drift:>7.2}",
                    joint_descs.get(joint).copied().unwrap_or(usize::MAX) as i64,
                    rest_t[0],
                    rest_t[1],
                    rest_t[2],
                    pose.translate[0],
                    pose.translate[1],
                    pose.translate[2],
                    pose.rotate[0],
                    pose.rotate[1],
                    pose.rotate[2],
                );
            }
        }
    }

    // Replaying from the pack is the end-to-end check: the same scripts, the
    // same joint-to-node pairing and the same rest poses, but reached through
    // the build-time tables rather than recomputed from the ROM. Any step of
    // that resolution going wrong shows up as a pose that differs.
    if let Some(path) = &pack_path {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let pack = ssb_rom::pack::Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;
        let (compared, worst) = replay_from_pack(&pack, &archive)?;
        println!();
        println!("replayed {compared} joint(s) from {}", path.display());
        if worst == 0.0 {
            println!("            every pose matches the ROM exactly");
        } else {
            return Err(format!("pack and ROM poses differ by up to {worst}").into());
        }

        let (bones, stretch, culprit) = check_bone_lengths(&pack)?;
        println!("posed    {bones} bone length(s) across every animation");
        // A rigid skeleton in f32 through a chain of matrix products; a unit or
        // two of rounding over a 300-unit fighter is arithmetic, not motion.
        if stretch < 1.0 {
            println!("            none stretched (worst {stretch:.3} units)");
        } else {
            return Err(format!("a bone changed length by {stretch:.1} units -- {culprit}").into());
        }
    }

    println!();
    if frames > 0 {
        println!("worst translation drift from the rest pose: {worst_drift:.2} units");
    }
    println!("{checked} animation(s) checked against their skeletons");
    if mismatched.is_empty() {
        println!("            every joint table matches its fighter's joint count");
        Ok(())
    } else {
        for m in mismatched.iter().take(20) {
            println!("  !! {m}");
        }
        Err(format!("{} joint table(s) disagree", mismatched.len()).into())
    }
}

/// A fighter's skeleton: the graph its `FTCommonPart` names, and the mask that
/// says which of that graph's descriptors become joints.
#[derive(Debug, Clone, Copy)]
struct Skeleton {
    file: u32,
    offset: u32,
    /// Descriptors in the graph.
    nodes: usize,
    /// `setup_parts`, bit *n* for descriptor *n*.
    mask: u64,
}

/// Plays every packed animation from the pack and from the ROM, and requires
/// the two to agree.
///
/// The ROM side reaches a joint's script by re-deriving everything: the
/// `FTCommonPart` record, the `setup_parts` mask, the pointer table. The pack
/// side just reads the tables the build wrote. Agreement on every frame of
/// every joint means that resolution survived being written down.
///
/// Returns `(joints compared, worst absolute difference)`.
fn replay_from_pack(
    pack: &ssb_rom::pack::Pack<'_>,
    archive: &Archive<'_>,
) -> Result<(usize, f32), Box<dyn std::error::Error>> {
    use ssb_rom::figatree as fg;
    use ssb_rom::pack::AnimJoint;

    let mut compared = 0usize;
    let mut worst = 0.0f32;
    for i in 0..pack.anim_count() {
        let Some(a) = pack.anim(i) else { continue };
        // Stage scenery shares this table but runs the 32-bit event stream, not
        // figatree's 16-bit one (RE-050, RE-051). Decoding one as the other
        // walks off the end of the file.
        if a.fighter == ssb_rom::pack::AnimDesc::STAGE {
            continue;
        }
        let Some(script) = pack.anim_script(&a) else {
            return Err(format!("animation {i} has no script bytes in the blob").into());
        };
        // The same bytes, fetched the long way round.
        let rom = archive.load(a.source_file)?;
        if rom.data != script {
            return Err(format!(
                "animation {i}: packed script differs from archive file {}",
                a.source_file
            )
            .into());
        }
        // Long enough for every movement animation; looping ones never end.
        let steps = 64;
        for j in 0..a.joint_count {
            let Some(joint) = pack.anim_joint(a.first_joint + j) else {
                continue;
            };
            if joint.script == AnimJoint::NO_SCRIPT || joint.node == AnimJoint::NO_NODE {
                continue;
            }
            let Some(node) = pack.node(joint.node) else {
                return Err(format!("animation {i} joint {j} names node {}", joint.node).into());
            };
            let rest = fg::JointPose {
                rotate: node.rest_rotate,
                translate: node.rest_translate,
                scale: node.rest_scale,
            };
            let mut from_pack = (fg::JointAnim::start(joint.script as usize, 0.0), rest);
            let mut from_rom = (fg::JointAnim::start(joint.script as usize, 0.0), rest);
            for _ in 0..steps {
                from_pack.0.tick(script, 1.0, &mut from_pack.1)?;
                from_rom.0.tick(&rom.data, 1.0, &mut from_rom.1)?;
                let d = from_pack
                    .1
                    .rotate
                    .iter()
                    .chain(&from_pack.1.translate)
                    .chain(&from_pack.1.scale)
                    .zip(
                        from_rom
                            .1
                            .rotate
                            .iter()
                            .chain(&from_rom.1.translate)
                            .chain(&from_rom.1.scale),
                    )
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0f32, f32::max);
                worst = worst.max(d);
            }
            compared += 1;
        }
    }
    Ok((compared, worst))
}

/// Every animation, every frame: no bone changes length unless the animation
/// moved it on purpose.
///
/// A skeleton poses by rotating joints, so the distance from a node to its
/// parent is fixed — *unless* that node's own translation tracks are animated,
/// which a few are. Master Hand is the clear case: it is a hand, its fingers
/// are placed by translation rather than rotation, and its root's rest
/// descriptor is overridden outright (RE-036). So the check skips a bone whose
/// child's translation has left its rest value, and holds every other one to
/// rigidity.
///
/// This is the invariant "the model exploded" would break, and it is checkable
/// without knowing what any pose is supposed to look like — which matters,
/// because judging a pose by eye on an untextured model at an arbitrary view
/// angle is how a working pipeline got mistaken for a broken one (RE-038).
///
/// Returns `(bones checked, worst length change in game units)`.
fn check_bone_lengths(
    pack: &ssb_rom::pack::Pack<'_>,
) -> Result<(usize, f32, String), Box<dyn std::error::Error>> {
    use ssb_rom::pack::{AnimJoint, NodeDesc};
    use ssb_rom::scene::Mat4;
    use ssb_rom::skeleton::{Skeleton, MAX_NODES};

    let scale = ssb_rom::pack::MODEL_SCALE;
    // A DObjDesc hierarchy is depth-capped well below this; the bound is only
    // here so a malformed parent chain cannot spin.
    const MAX_DEPTH: usize = 32;
    let mut checked = 0usize;
    let mut worst = 0.0f32;
    let mut culprit = String::new();
    let mut bad: BTreeSet<String> = BTreeSet::new();

    for i in 0..pack.anim_count() {
        let Some(a) = pack.anim(i) else { continue };
        // Stage scenery shares this table but runs the 32-bit event stream, not
        // figatree's 16-bit one (RE-050, RE-051). Decoding one as the other
        // walks off the end of the file.
        if a.fighter == ssb_rom::pack::AnimDesc::STAGE {
            continue;
        }
        let Some(script) = pack.anim_script(&a) else {
            continue;
        };
        let Some(node0) = (0..a.joint_count)
            .filter_map(|j| pack.anim_joint(a.first_joint + j))
            .map(|j| j.node)
            .find(|&n| n != AnimJoint::NO_NODE)
        else {
            continue;
        };
        let Some(object) = (0..pack.object_count())
            .filter_map(|o| pack.object(o))
            .find(|o| node0 >= o.first_node && node0 < o.first_node + o.node_count)
        else {
            continue;
        };

        let bone = |m: &[Mat4], k: usize| -> Option<f32> {
            let n = pack.node(object.first_node + k as u32)?;
            if n.parent == NodeDesc::NO_PARENT {
                return None;
            }
            let p = (n.parent - object.first_node) as usize;
            let (a, b) = (m[k].translation(), m.get(p)?.translation());
            Some(
                (((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt())
                    * scale,
            )
        };

        let mut rest = [Mat4::IDENTITY; MAX_NODES];
        Skeleton::new().compose(pack, &object, &mut rest);

        let mut sk = Skeleton::new();
        sk.start(pack, &a, 0.0, 1.0);
        // Long enough to cover the longest movement animation and then some.
        for _ in 0..48 {
            sk.tick(script)?;
            let mut m = [Mat4::IDENTITY; MAX_NODES];
            sk.compose(pack, &object, &mut m);
            for k in 0..object.node_count as usize {
                // Only rotation preserves a bone. Two things legitimately do
                // not: animating this node's own translation, and animating
                // the *scale* of anything above it -- a parent's scale
                // multiplies its children's offsets, which is how Kirby and
                // Jigglypuff squash. Walk up and skip the bone if either
                // applies.
                let mut at = object.first_node + k as u32;
                let mut deformed = false;
                for _ in 0..MAX_DEPTH {
                    let Some(n) = pack.node(at) else { break };
                    if let Some(p) = joint_pose(&sk, at) {
                        if at == object.first_node + k as u32
                            && (0..3).any(|c| (p.translate[c] - n.rest_translate[c]).abs() > 0.01)
                        {
                            deformed = true;
                        }
                        if (0..3).any(|c| (p.scale[c] - n.rest_scale[c]).abs() > 0.001) {
                            deformed = true;
                        }
                    }
                    if n.parent == NodeDesc::NO_PARENT {
                        break;
                    }
                    at = n.parent;
                }
                if deformed {
                    continue;
                }
                let (Some(r), Some(p)) = (bone(&rest, k), bone(&m, k)) else {
                    continue;
                };
                checked += 1;
                let d = (p - r).abs();
                if d > 1.0 {
                    bad.insert(format!("fighter {} slot {}", a.fighter, a.slot));
                }
                if d > worst {
                    worst = d;
                    culprit = format!(
                        "fighter {} slot {} node {k}: {r:.1} -> {p:.1}",
                        a.fighter, a.slot
                    );
                }
            }
        }
    }
    if !bad.is_empty() {
        culprit = format!(
            "{} animation(s): {}",
            bad.len(),
            bad.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    Ok((checked, worst, culprit))
}

/// The translation a skeleton currently holds for a node, if it drives one.
fn joint_pose(
    sk: &ssb_rom::skeleton::Skeleton,
    node: u32,
) -> Option<&ssb_rom::figatree::JointPose> {
    (0..sk.joint_count())
        .find(|&j| sk.joint_node(j) == Some(node))
        .and_then(|j| sk.pose(j))
}

/// Reads a figatree's joint pointer table.
///
/// The table's length is not stored: the first non-null pointer is the offset
/// of the first script, which is exactly where the table ends.
fn joint_table(data: &[u8]) -> Option<Vec<u32>> {
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
    };
    let mut first = 0;
    let mut at = 0;
    while let Some(p) = word(at) {
        if p != 0 {
            first = p;
            break;
        }
        at += 4;
    }
    if first == 0 || first % 4 != 0 || first as usize > data.len() {
        return None;
    }
    (0..first as usize / 4).map(|i| word(i * 4)).collect()
}

// ---------------------------------------------------------------------------
// texgen census
// ---------------------------------------------------------------------------

/// Effective RSP texture-coordinate generation implied by a raw geometry-mode
/// word. Deliberately re-derived here rather than imported from
/// `ssb_rom::mesh`: the census exists to *check* the converter, so it must not
/// share the code under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Texgen {
    None,
    Regular,
    Linear,
}

const GM_TEXTURE_GEN: u32 = 0x0004_0000;
const GM_TEXTURE_GEN_LINEAR: u32 = 0x0008_0000;
const GM_LIGHTING: u32 = 0x0002_0000;
/// `sSYRdpResetDisplayList`'s per-frame baseline: `G_ZBUFFER | G_SHADE |
/// G_CULL_BACK | G_SHADING_SMOOTH`.
const GM_RESET_DEFAULT: u32 = 0x0000_0001 | 0x0000_0004 | 0x0000_0400 | 0x0020_0000;

impl Texgen {
    fn of(gm: u32) -> Self {
        if gm & GM_TEXTURE_GEN == 0 {
            Texgen::None
        } else if gm & GM_TEXTURE_GEN_LINEAR != 0 {
            Texgen::Linear
        } else {
            Texgen::Regular
        }
    }
}

/// What was true when a vertex was written into the cache.
#[derive(Debug, Clone, Copy, PartialEq)]
struct VtxLoadState {
    geometry_mode: u32,
    tex_scale: (u16, u16),
    /// Draw-sequence step (scene-graph node index, or list ordinal for a
    /// discovered list) the load happened in. Compared against the step the
    /// triangle draws in to find cache reuse across node boundaries.
    step: usize,
    /// Display-list offset the `G_VTX` itself came from.
    dl: u32,
    /// The scene-graph node whose matrix was in force (`PlannedList::space`),
    /// or `None` for the object root/a discovered list with no graph.
    space: Option<usize>,
    /// That node's world transform (`PlannedList::world`), the RE-225/`R2.1`/
    /// T1 "stable load model-matrix identity" -- F3DEX generates texture
    /// coordinates at `G_VTX` time, so this is what the real hardware would
    /// have transformed the vertex's normal with.
    world: ssb_rom::scene::Mat4,
}

/// The bound tile, as far as it affects generated texture coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct TileState {
    fmt: Option<(u8, u8)>,
    dims: Option<(u16, u16)>,
    origin: (u16, u16),
    mask: (u8, u8),
    cm: (u8, u8),
    /// `R2.1`/T6: `G_SETTILE`'s `shift_s`/`shift_t`. RE-223 measured these
    /// zero for every real render-tile-0 `G_SETTILE` archive-wide; carried
    /// here so a texgen-specific audit can confirm that invariant holds for
    /// the texgen-bound subset too, not just assume it transfers.
    shift: (u8, u8),
}

#[derive(Default)]
struct TexgenCensus {
    triangles: u64,
    texgen_triangles: u64,
    /// Effective draw-time mode of every triangle drawn under generation.
    by_mode: BTreeMap<Texgen, u64>,
    /// Every raw `(G_TEXTURE_GEN, G_TEXTURE_GEN_LINEAR)` bit pair observed at
    /// a draw, however reached.
    raw_bits: BTreeMap<(bool, bool), u64>,
    /// Raw bit pairs observed at any point in a walk, including states no
    /// triangle was drawn under.
    raw_bits_transient: BTreeMap<(bool, bool), u64>,
    /// Triangles whose three vertices were not all loaded under the same
    /// effective mode.
    mixed_load_mode: u64,
    /// Triangles drawn under generation whose vertices were loaded under a
    /// different effective mode (loaded before enable, or after disable).
    load_draw_mode_mismatch: u64,
    /// Triangles whose three vertices were not all loaded under the same
    /// `G_TEXTURE` scale.
    mixed_load_scale: u64,
    /// Texgen triangles whose vertices' load-time `G_TEXTURE` scale differs
    /// from the scale in force at the draw.
    load_draw_scale_mismatch: u64,
    /// Texgen triangles using a vertex loaded in an earlier draw step.
    cross_step_vertices: u64,
    /// Texgen triangles using a vertex loaded by a different display list.
    cross_list_vertices: u64,
    /// `R2.1`/T1 (RE-225): among vertices loaded in an earlier step than the
    /// triangle they draw in, how the load's node/space compares to the
    /// draw's. A vertex transformed at `G_VTX` time under one node's matrix
    /// but drawn as part of a triangle whose "current" node differs is only
    /// safe to cache/reuse across that boundary if the two matrices agree
    /// on the normal-relevant 3x3 transform.
    same_node_reuse: u64,
    cross_list_same_node_reuse: u64,
    cross_node_equivalent_transform_reuse: u64,
    cross_node_differing_transform_reuse: u64,
    /// Distinct `G_TEXTURE` scales in force at a texgen draw.
    texgen_scales: BTreeMap<(u16, u16), u64>,
    /// Distinct tile setups bound by a texgen draw.
    texgen_tiles: BTreeMap<TileState, u64>,
    /// `R2.1`/T6: the same tile setups, split by effective texgen mode
    /// (`Regular` vs `Linear` generate different UVs against the same
    /// bound tile, so the audit needs the addressing state per mode, not
    /// only pooled across both).
    texgen_tiles_by_mode: BTreeMap<(Texgen, TileState), u64>,
    /// `R2.1`/T6: `G_TEXTURE` scale in force at a texgen draw, split by mode.
    texgen_scales_by_mode: BTreeMap<(Texgen, (u16, u16)), u64>,
    /// `R2.1`/T6: raw `G_LIGHTING` state observed at each `G_VTX` executed
    /// while texgen is active, split by mode. F3DEX resolves lighting and
    /// texgen together at vertex-load time, so this is the load-time signal
    /// that decides whether a texgen-bound vertex used a lit or raw normal.
    texgen_vtx_lighting: BTreeMap<(Texgen, bool), u64>,
    /// `R2.1`/T7: the scale and tile actually paired together at a real
    /// texgen draw -- `texgen_scales_by_mode`/`texgen_tiles_by_mode` (T6)
    /// keep these as two separate maps, which loses which scale belongs to
    /// which tile when a mode has more than one of either. T7's addressing
    /// comparison needs the real pairing, since `address_axis`'s far edge
    /// and `psp_lowering_axis`'s period both come from the tile while the
    /// generated coordinate's magnitude comes from the scale.
    texgen_materials_by_mode: BTreeMap<(Texgen, (u16, u16), TileState), u64>,
    /// Files, and (file, graph, node) sites, that draw texgen at all.
    texgen_files: BTreeSet<u32>,
    texgen_sites: BTreeSet<(u32, u32, usize, u32)>,
}

/// Per-list walker state for the census.
struct TexgenWalk {
    geometry_mode: u32,
    tex_scale: (u16, u16),
    cache: [Option<VtxLoadState>; 32],
    tile: TileState,
    step: usize,
    dl: u32,
    depth: u32,
    /// The scene-graph node whose matrix is in force for the whole step
    /// (`PlannedList::space`); constant across a step's own list and any
    /// lists it calls, since no `G_MTX` is modeled here (RE-225).
    space: Option<usize>,
    /// That node's world transform (`PlannedList::world`).
    world: ssb_rom::scene::Mat4,
    /// The current node's `MObj` chain, indexed by graphics-heap entry.
    mobjs: Vec<ssb_rom::mobj::MObjMaterial>,
}

impl TexgenWalk {
    fn new() -> Self {
        TexgenWalk {
            geometry_mode: GM_RESET_DEFAULT,
            tex_scale: (0xFFFF, 0xFFFF),
            cache: [None; 32],
            tile: TileState::default(),
            step: 0,
            dl: 0,
            depth: 0,
            space: None,
            world: ssb_rom::scene::Mat4::IDENTITY,
            mobjs: Vec::new(),
        }
    }

    fn walk(
        &mut self,
        cmds: &[ssb_rom::dl::Cmd],
        file: &ssb_rom::archive::File,
        graph: u32,
        node: usize,
        census: &mut TexgenCensus,
        verbose: bool,
    ) {
        use ssb_rom::dl::Cmd;

        for cmd in cmds {
            match *cmd {
                Cmd::GeometryMode { clear, set } => {
                    self.geometry_mode &= !clear;
                    self.geometry_mode |= set;
                    let gm = self.geometry_mode;
                    *census
                        .raw_bits_transient
                        .entry((gm & GM_TEXTURE_GEN != 0, gm & GM_TEXTURE_GEN_LINEAR != 0))
                        .or_default() += 1;
                }
                Cmd::Texture {
                    scale_s, scale_t, ..
                } => self.tex_scale = (scale_s, scale_t),
                Cmd::SetTile {
                    format,
                    size,
                    tile: 0,
                    mask_s,
                    mask_t,
                    cm_s,
                    cm_t,
                    shift_s,
                    shift_t,
                    ..
                } => {
                    self.tile.fmt = Some((format, size));
                    self.tile.mask = (mask_s, mask_t);
                    self.tile.cm = (cm_s, cm_t);
                    self.tile.shift = (shift_s, shift_t);
                }
                Cmd::SetTileSize {
                    tile: 0,
                    uls,
                    ult,
                    lrs,
                    lrt,
                } => {
                    self.tile.dims = Some((
                        ((lrs.saturating_sub(uls)) >> 2) + 1,
                        ((lrt.saturating_sub(ult)) >> 2) + 1,
                    ));
                    self.tile.origin = (uls, ult);
                }
                Cmd::Vtx {
                    count, dest_index, ..
                } => {
                    let mode = Texgen::of(self.geometry_mode);
                    if mode != Texgen::None {
                        *census
                            .texgen_vtx_lighting
                            .entry((mode, self.geometry_mode & GM_LIGHTING != 0))
                            .or_default() += 1;
                    }
                    let load = VtxLoadState {
                        geometry_mode: self.geometry_mode,
                        tex_scale: self.tex_scale,
                        step: self.step,
                        dl: self.dl,
                        space: self.space,
                        world: self.world,
                    };
                    for i in 0..count as usize {
                        let slot = dest_index as usize + i;
                        if slot < self.cache.len() {
                            self.cache[slot] = Some(load);
                        }
                    }
                }
                Cmd::Tri1(t) => self.tri(t, file, graph, node, census, verbose),
                Cmd::Tri2(a, b) => {
                    self.tri(a, file, graph, node, census, verbose);
                    self.tri(b, file, graph, node, census, verbose);
                }
                Cmd::Call(addr) | Cmd::Branch(addr) => {
                    let tail = matches!(cmd, Cmd::Branch(_));
                    // A call into the runtime graphics heap names one of the
                    // node's `MObj` materials, which can override the tile
                    // rectangle and the `G_TEXTURE` scale exactly as an
                    // in-list command would (`mesh.rs`'s `apply_mobj`). The
                    // census has to replay that or it under-reports both.
                    if addr.segment() == ssb_rom::mobj::GRAPHICS_HEAP_SEGMENT {
                        let index = (addr.offset() / 8) as usize;
                        if let Some(m) = self.mobjs.get(index) {
                            if let Some(scale) = m.tex_scale {
                                self.tex_scale = scale;
                            }
                            if let Some((uls, ult, lrs, lrt)) = m.tile0_uv {
                                self.tile.dims = Some((
                                    ((lrs.saturating_sub(uls)) >> 2) + 1,
                                    ((lrt.saturating_sub(ult)) >> 2) + 1,
                                ));
                                self.tile.origin = (uls, ult);
                            }
                        }
                    }
                    if self.depth < 8 && addr.segment() == 0 {
                        let at = addr.0 as usize;
                        if at < file.data.len() {
                            if let Ok(sub) =
                                ssb_rom::dl::decode_list_at(&file.data[at..], at as u32)
                            {
                                let outer = self.dl;
                                let outer_depth = self.depth;
                                self.dl = at as u32;
                                self.depth += 1;
                                self.walk(&sub, file, graph, node, census, verbose);
                                self.dl = outer;
                                self.depth = outer_depth;
                            }
                        }
                    }
                    if tail {
                        break;
                    }
                }
                Cmd::End => break,
                _ => {}
            }
        }
    }

    fn tri(
        &self,
        tri: [u8; 3],
        file: &ssb_rom::archive::File,
        graph: u32,
        node: usize,
        census: &mut TexgenCensus,
        verbose: bool,
    ) {
        census.triangles += 1;
        let draw = Texgen::of(self.geometry_mode);
        *census
            .raw_bits
            .entry((
                self.geometry_mode & GM_TEXTURE_GEN != 0,
                self.geometry_mode & GM_TEXTURE_GEN_LINEAR != 0,
            ))
            .or_default() += 1;
        if draw == Texgen::None {
            return;
        }

        census.texgen_triangles += 1;
        *census.by_mode.entry(draw).or_default() += 1;
        *census.texgen_scales.entry(self.tex_scale).or_default() += 1;
        *census.texgen_tiles.entry(self.tile).or_default() += 1;
        *census
            .texgen_scales_by_mode
            .entry((draw, self.tex_scale))
            .or_default() += 1;
        *census
            .texgen_tiles_by_mode
            .entry((draw, self.tile))
            .or_default() += 1;
        *census
            .texgen_materials_by_mode
            .entry((draw, self.tex_scale, self.tile))
            .or_default() += 1;
        census.texgen_files.insert(file.id);
        census.texgen_sites.insert((file.id, graph, node, self.dl));

        let loads: Vec<Option<VtxLoadState>> = tri
            .iter()
            .map(|&s| self.cache.get(s as usize).copied().flatten())
            .collect();
        let modes: Vec<Texgen> = loads
            .iter()
            .filter_map(|l| l.map(|l| Texgen::of(l.geometry_mode)))
            .collect();
        let scales: Vec<(u16, u16)> = loads
            .iter()
            .filter_map(|l| l.map(|l| l.tex_scale))
            .collect();

        if modes.windows(2).any(|w| w[0] != w[1]) {
            census.mixed_load_mode += 1;
        }
        if modes.iter().any(|&m| m != draw) {
            census.load_draw_mode_mismatch += 1;
            if verbose {
                println!(
                    "  file {:>4} graph {:#x} node {:<3} dl {:#x}: draw {draw:?}, loads {modes:?}",
                    file.id, graph, node, self.dl
                );
            }
        }
        if scales.windows(2).any(|w| w[0] != w[1]) {
            census.mixed_load_scale += 1;
        }
        if scales.iter().any(|&s| s != self.tex_scale) {
            census.load_draw_scale_mismatch += 1;
            if verbose {
                println!(
                    "  file {:>4} graph {:#x} node {:<3} dl {:#x}: draw scale {:?}, loads {scales:?}",
                    file.id, graph, node, self.dl, self.tex_scale
                );
            }
        }
        if loads.iter().flatten().any(|l| l.step != self.step) {
            census.cross_step_vertices += 1;
        }
        if loads.iter().flatten().any(|l| l.dl != self.dl) {
            census.cross_list_vertices += 1;
        }

        for l in loads.iter().flatten() {
            if l.step == self.step {
                continue;
            }
            if l.space == self.space {
                if l.dl == self.dl {
                    census.same_node_reuse += 1;
                } else {
                    census.cross_list_same_node_reuse += 1;
                }
            } else if normal_transform_equivalent(&l.world, &self.world) {
                census.cross_node_equivalent_transform_reuse += 1;
            } else {
                census.cross_node_differing_transform_reuse += 1;
                if verbose {
                    println!(
                        "  file {:>4} graph {:#x} node {:<3} dl {:#x}: cross-node differing-transform reuse (load space {:?}, draw space {:?})",
                        file.id, graph, node, self.dl, l.space, self.space
                    );
                }
            }
        }
    }
}

/// The 3x3 linear part of `m` (rotation + scale, no translation), in
/// column-major order matching [`ssb_rom::scene::Mat4`]'s own layout.
fn linear3x3(m: &ssb_rom::scene::Mat4) -> [f32; 9] {
    let d = m.0;
    [d[0], d[1], d[2], d[4], d[5], d[6], d[8], d[9], d[10]]
}

/// `R2.1`/T1 (RE-225): whether two node transforms generate the same texgen
/// coordinates for a shared vertex normal -- a bit-identical 3x3 linear part
/// (translation, which does not affect a normal, ignored entirely), or one
/// that differs from the other only by a positive uniform scale (a squash/
/// stretch keyframe changes vertex position but not normal direction). A
/// non-uniform scale, or a genuinely different rotation, is not equivalent.
fn normal_transform_equivalent(a: &ssb_rom::scene::Mat4, b: &ssb_rom::scene::Mat4) -> bool {
    let ea = linear3x3(a);
    let eb = linear3x3(b);
    if ea == eb {
        return true;
    }
    let Some((idx, _)) = eb
        .iter()
        .enumerate()
        .max_by(|x, y| x.1.abs().total_cmp(&y.1.abs()))
    else {
        return false;
    };
    if eb[idx].abs() < 1e-6 {
        return ea.iter().all(|v| v.abs() < 1e-6);
    }
    let scale = ea[idx] / eb[idx];
    if scale <= 0.0 {
        return false;
    }
    ea.iter()
        .zip(eb.iter())
        .all(|(&x, &y)| (x - scale * y).abs() <= 1e-4 * (1.0 + x.abs()))
}

/// Archive-wide census of the state every texgen triangle is drawn under.
///
/// The question it answers is whether `G_TEXTURE_GEN` state can safely stay
/// *primitive*-level in this port. F3DEX generates texture coordinates during
/// `G_VTX` processing, so the state that matters is the one in force when each
/// vertex was loaded — which only equals the state at the draw if no list ever
/// loads a vertex under one texgen/`G_TEXTURE` state and draws it under
/// another. This measures exactly that, walking the same two list populations
/// `pack` converts: every scene graph's planned draw order (state threaded
/// across nodes, as `convert_sequence` does), then the discovered root lists no
/// graph claims.
/// Reports every texgen primitive a built pack carries, with the texture
/// dimensions the GE will normalise its generated coordinates against.
fn report_packed_texgen(path: &Path) -> Res {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = ssb_rom::pack::Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;
    println!("packed texgen primitives ({})", path.display());
    println!(
        "  {:>6}  {:>5}  {:>5}  {:>6}  {:>4}  {:>7}  {:>9}  {:>9}",
        "prim", "file", "tris", "linear", "tex", "uploaded", "scale S/T", "origin"
    );
    // Which archive file each primitive's own mesh came from, so a scene can
    // be checked for texgen content without guessing.
    let mut prim_file: BTreeMap<u32, u32> = BTreeMap::new();
    for i in 0..pack.mesh_count() {
        let Some(m) = pack.mesh(i) else { continue };
        for p in m.first_prim..m.first_prim + m.prim_count {
            prim_file.insert(p, m.source_file);
        }
    }
    let mut count = 0;
    for i in 0..pack.prim_count() {
        let Some(p) = pack.prim(i) else { continue };
        if p.flags & ssb_rom::pack::flags::TEXTURE_GEN == 0 {
            continue;
        }
        count += 1;
        let linear = p.flags & ssb_rom::pack::flags::TEXTURE_GEN_LINEAR != 0;
        let t = pack.texture(p.texture);
        let dims = t.as_ref().map_or("none".to_string(), |t| {
            let (w, h) = ssb_rom::psp_texture::ge_texture_dims(t.width as u32, t.height as u32);
            format!("{w}x{h}")
        });
        println!(
            "  {:>6}  {:>5}  {:>5}  {:>6}  {:>4}  {:>7}  {:#06x}/{:#06x}  {:>4}/{:<4}",
            i,
            prim_file.get(&i).copied().unwrap_or(u32::MAX),
            p.index_count / 3,
            linear,
            p.texture,
            dims,
            p.texgen_scale_s,
            p.texgen_scale_t,
            p.texgen_origin_s,
            p.texgen_origin_t
        );
    }
    println!("  {count} texgen primitive(s)");
    println!();
    Ok(())
}

/// Walks every graph-planned list and unclaimed discovered root list in the
/// archive, building the `TexgenCensus` `texgen()` and its `SSB64_ROM`-gated
/// tests both report from. Shared so a test can measure the same real
/// archive-wide walk the CLI command prints, instead of a second heuristic.
fn build_texgen_census(
    archive: &Archive,
    loaded: &Loaded,
    only_file: Option<u32>,
    verbose: bool,
) -> (TexgenCensus, usize, usize) {
    let mut census = TexgenCensus::default();
    let mut graph_lists = 0usize;
    let mut discovered_lists = 0usize;

    for id in 0..archive.len() as u32 {
        if only_file.is_some_and(|f| f != id) {
            continue;
        }
        let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
            continue;
        };

        let graphs: &[ssb_rom::scene::SceneGraph] =
            loaded.graphs.get(&id).map_or(&[], Vec::as_slice);
        let resolver = ssb_rom::scene::DlResolver::new(file);
        let mut authoritative: BTreeSet<u32> = BTreeSet::new();

        for (gi, graph) in graphs.iter().enumerate() {
            let plan = plan_draw_order(graph, &resolver);
            let materials = loaded.materials(file, graph);
            // One walker for the whole graph: the RDP/RSP state and the vertex
            // cache persist across a graph's nodes exactly as they do on
            // hardware, which is the only way a cross-node reuse can show up.
            let mut walk = TexgenWalk::new();
            for (step, p) in plan.iter().enumerate() {
                if p.dl == NO_LIST {
                    continue;
                }
                authoritative.insert(p.dl);
                graph_lists += 1;
                let Some(cmds) = file
                    .data
                    .get(p.dl as usize..)
                    .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                else {
                    continue;
                };
                walk.step = step;
                walk.dl = p.dl;
                walk.space = p.space;
                walk.world = p.world;
                walk.mobjs = materials.get(p.node).cloned().unwrap_or_default();
                walk.walk(&cmds, file, gi as u32, p.node, &mut census, verbose);
            }
        }

        let all = ssb_rom::scan::find_root_display_lists(file);
        let called: BTreeSet<u32> = all.iter().flat_map(|d| d.referenced_lists()).collect();
        for dl in all
            .iter()
            .filter(|d| !called.contains(&d.offset) && !authoritative.contains(&d.offset))
        {
            discovered_lists += 1;
            let mut walk = TexgenWalk::new();
            walk.dl = dl.offset;
            walk.walk(
                &dl.commands,
                file,
                u32::MAX,
                usize::MAX,
                &mut census,
                verbose,
            );
        }
    }

    (census, graph_lists, discovered_lists)
}

/// `PLAN.md` R2.1/T10: the addressing check `texgen_addressing_census_against_real_archive_materials`
/// already proved against `SSB64_ROM`, factored out so `romtool texgen
/// --verify` runs the exact same walk against any ROM, not a second copy of
/// the logic that could drift from what the test actually checks.
struct TexgenAddressingReport {
    materials_examined: u64,
    axis_instances: u64,
    non_clamp_axis_instances: u64,
    diverging_axis_instances: u64,
    diverging_at_non_extreme_dot: u64,
    diverging_combos: BTreeSet<(Texgen, u16, u16, u8)>,
}

fn verify_texgen_addressing(census: &TexgenCensus) -> TexgenAddressingReport {
    let mut report = TexgenAddressingReport {
        materials_examined: 0,
        axis_instances: 0,
        non_clamp_axis_instances: 0,
        diverging_axis_instances: 0,
        diverging_at_non_extreme_dot: 0,
        diverging_combos: BTreeSet::new(),
    };

    for &(mode, scale, tile) in census.texgen_materials_by_mode.keys() {
        let Some(dims) = tile.dims else { continue };
        report.materials_examined += 1;

        for axis in 0..2usize {
            report.axis_instances += 1;
            let scale_axis = if axis == 0 { scale.0 } else { scale.1 };
            let origin_axis = if axis == 0 {
                tile.origin.0
            } else {
                tile.origin.1
            };
            let dim_axis = if axis == 0 { dims.0 } else { dims.1 };
            let mask_axis = if axis == 0 { tile.mask.0 } else { tile.mask.1 };
            let cm_axis = if axis == 0 { tile.cm.0 } else { tile.cm.1 };
            let mirror = cm_axis & 1 != 0;
            let clamp_bit = cm_axis & 2 != 0;
            if !clamp_bit {
                report.non_clamp_axis_instances += 1;
                continue;
            }

            let model = ssb_rom::n64_addressing::TileAxis {
                shift: 0,
                origin_q2: 0,
                far_edge_q2: (dim_axis as i32 - 1) << 2,
                mask: mask_axis,
                mirror,
                clamp_bit,
            };
            let period = 1u32 << mask_axis;

            for n in -127i8..=127 {
                let mut normal = [0i8; 3];
                normal[0] = n;
                let basis_s = if axis == 0 { [1.0, 0.0, 0.0] } else { [0.0; 3] };
                let basis_t = if axis == 1 { [1.0, 0.0, 0.0] } else { [0.0; 3] };
                let (u, v) = match mode {
                    Texgen::Regular => ssb_rom::psp_texture::regular_texgen_uv(
                        normal,
                        basis_s,
                        basis_t,
                        scale_axis,
                        scale_axis,
                        origin_axis,
                        origin_axis,
                        true,
                        true,
                    ),
                    Texgen::Linear => ssb_rom::psp_texture::linear_texgen_uv(
                        normal,
                        basis_s,
                        basis_t,
                        scale_axis,
                        scale_axis,
                        origin_axis,
                        origin_axis,
                        true,
                        true,
                    ),
                    Texgen::None => unreachable!("texgen_materials_by_mode never keys None"),
                };
                let coord = (if axis == 0 { u } else { v }) as i32;

                let hw = ssb_rom::n64_addressing::address_axis(&model, coord);
                let psp = ssb_rom::n64_addressing::psp_lowering_axis(
                    coord,
                    period,
                    dim_axis as u32,
                    mirror,
                    clamp_bit,
                );
                if hw != psp {
                    report.diverging_axis_instances += 1;
                    report
                        .diverging_combos
                        .insert((mode, scale_axis, dim_axis, mask_axis));
                    if n != 127 {
                        report.diverging_at_non_extreme_dot += 1;
                    }
                }
            }
        }
    }

    report
}

/// `PLAN.md` R2.1/T10: the correctness-critical invariants `romtool texgen
/// --verify` fails on -- each one is a real ROM measurement this project has
/// already pinned as a regression baseline elsewhere (RE-225/RE-230/RE-231/
/// RE-232), reproduced here against whatever ROM is passed so CI or a
/// developer can catch a regression without reaching for `cargo test` and
/// `SSB64_ROM`.
fn verify_texgen(census: &TexgenCensus) -> Vec<String> {
    let mut failures = Vec::new();

    if census.load_draw_mode_mismatch != 0 {
        failures.push(format!(
            "load_draw_mode_mismatch = {} (want 0): a texgen triangle's vertices were loaded \
             under a different effective mode than the draw itself",
            census.load_draw_mode_mismatch
        ));
    }
    if census.load_draw_scale_mismatch != 0 {
        failures.push(format!(
            "load_draw_scale_mismatch = {} (want 0): a texgen triangle's vertices were loaded \
             under a different G_TEXTURE scale than the draw itself",
            census.load_draw_scale_mismatch
        ));
    }
    for (tile, n) in &census.texgen_tiles {
        if tile.shift != (0, 0) {
            failures.push(format!(
                "texgen-bound tile shift {:?} used by {n} triangle(s) (want (0, 0), \
                 PLAN.md R2.1/T6): N64 tile shifting is not implemented",
                tile.shift
            ));
        }
    }

    let addressing = verify_texgen_addressing(census);
    if addressing.non_clamp_axis_instances != 0 {
        failures.push(format!(
            "non_clamp_axis_instances = {} (want 0, PLAN.md R2.1/T7): a real texgen tile now \
             has a non-clamp axis, which this addressing check does not cover",
            addressing.non_clamp_axis_instances
        ));
    }
    if addressing.diverging_axis_instances != 0 {
        failures.push(format!(
            "diverging_axis_instances = {} (want 0, PLAN.md R2.1/T7a, RE-232): generated \
             texgen coordinates disagree with the hardware addressing model on {} distinct \
             (mode, scale, dim, mask) combination(s)",
            addressing.diverging_axis_instances,
            addressing.diverging_combos.len()
        ));
    }
    if addressing.diverging_at_non_extreme_dot != 0 {
        failures.push(format!(
            "diverging_at_non_extreme_dot = {} (want 0): an addressing divergence now reaches \
             beyond the sweep's dot=+1 extreme, a materially larger gap than RE-231 measured",
            addressing.diverging_at_non_extreme_dot
        ));
    }

    failures
}

fn texgen(path: &Path, args: &[&str]) -> Res {
    let mut only_file: Option<u32> = None;
    let mut verbose = false;
    let mut pack_path: Option<PathBuf> = None;
    let mut verify = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            "--lines" => verbose = true,
            "--pack" => pack_path = it.next().map(PathBuf::from),
            "--verify" => verify = true,
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    // The other half of the census: what a built pack actually carries for
    // the same primitives, so the ROM measurement above can be checked
    // against the state the renderer will really see.
    if let Some(pack_path) = &pack_path {
        report_packed_texgen(pack_path)?;
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    let loaded = load_all(&archive);

    let (census, graph_lists, discovered_lists) =
        build_texgen_census(&archive, &loaded, only_file, verbose);

    println!("texgen census");
    println!("  graph-planned lists      {graph_lists}");
    println!("  discovered root lists    {discovered_lists}");
    println!("  triangles                {}", census.triangles);
    println!("  texgen triangles         {}", census.texgen_triangles);
    println!("  files drawing texgen     {}", census.texgen_files.len());
    println!("  texgen draw sites        {}", census.texgen_sites.len());
    println!();
    println!("raw geometry bits at a draw (GEN, LINEAR) -> triangles");
    for ((gen, lin), n) in &census.raw_bits {
        println!("  ({}, {})  {n}", *gen as u8, *lin as u8);
    }
    println!();
    println!("raw geometry bits ever reached (GEN, LINEAR) -> geometry-mode commands");
    for ((gen, lin), n) in &census.raw_bits_transient {
        println!("  ({}, {})  {n}", *gen as u8, *lin as u8);
    }
    println!();
    println!("effective mode of texgen triangles");
    for (mode, n) in &census.by_mode {
        println!("  {mode:?}  {n}");
    }
    println!();
    println!("load-vs-draw invariance (texgen triangles)");
    println!(
        "  vertices loaded under mixed modes   {}",
        census.mixed_load_mode
    );
    println!(
        "  load mode differs from draw mode    {}",
        census.load_draw_mode_mismatch
    );
    println!(
        "  vertices loaded under mixed scales  {}",
        census.mixed_load_scale
    );
    println!(
        "  load scale differs from draw scale  {}",
        census.load_draw_scale_mismatch
    );
    println!(
        "  uses a vertex from an earlier step  {}",
        census.cross_step_vertices
    );
    println!(
        "  uses a vertex from another list     {}",
        census.cross_list_vertices
    );
    println!();
    println!("model-space reuse of an earlier-step vertex (R2.1/T1, RE-225)");
    println!(
        "  same node, same list                {}",
        census.same_node_reuse
    );
    println!(
        "  same node, cross list                {}",
        census.cross_list_same_node_reuse
    );
    println!(
        "  cross node, equivalent transform     {}",
        census.cross_node_equivalent_transform_reuse
    );
    println!(
        "  cross node, differing transform      {}",
        census.cross_node_differing_transform_reuse
    );
    println!();
    println!("G_TEXTURE scale in force at a texgen draw");
    for ((s, t), n) in &census.texgen_scales {
        println!("  ({s:#06x}, {t:#06x})  {n}");
    }
    println!();
    println!("tile setup bound by a texgen draw");
    for (tile, n) in &census.texgen_tiles {
        println!(
            "  fmt {:?} dims {:?} origin {:?} mask {:?} shift {:?} cm {:?}  {n}",
            tile.fmt, tile.dims, tile.origin, tile.mask, tile.shift, tile.cm
        );
    }
    println!();
    println!("R2.1/T6: G_TEXTURE scale in force at a texgen draw, by mode");
    for ((mode, (s, t)), n) in &census.texgen_scales_by_mode {
        println!("  {mode:?}  ({s:#06x}, {t:#06x})  {n}");
    }
    println!();
    println!("R2.1/T6: tile setup bound by a texgen draw, by mode");
    for ((mode, tile), n) in &census.texgen_tiles_by_mode {
        println!(
            "  {mode:?}  fmt {:?} dims {:?} origin {:?} mask {:?} shift {:?} cm {:?}  {n}",
            tile.fmt, tile.dims, tile.origin, tile.mask, tile.shift, tile.cm
        );
    }
    println!();
    println!("R2.1/T6: raw G_LIGHTING at a texgen G_VTX, by mode");
    for ((mode, lit), n) in &census.texgen_vtx_lighting {
        println!("  {mode:?}  lit {lit}  {n}");
    }
    if !census.texgen_sites.is_empty() {
        println!();
        println!("texgen draw sites (file, graph, node, dl)");
        for (file, graph, node, dl) in &census.texgen_sites {
            let graph = if *graph == u32::MAX {
                "scan".to_string()
            } else {
                graph.to_string()
            };
            let node = if *node == usize::MAX {
                "-".to_string()
            } else {
                node.to_string()
            };
            println!("  {file:>4}  {graph:>4}  {node:>4}  {dl:#x}");
        }
    }

    if verify {
        println!();
        let failures = verify_texgen(&census);
        if failures.is_empty() {
            println!("verify: PASS (correctness-critical texgen invariants hold)");
        } else {
            println!("verify: FAIL ({} violation(s))", failures.len());
            for f in &failures {
                println!("  - {f}");
            }
            return Err(format!(
                "{} correctness-critical texgen violation(s)",
                failures.len()
            )
            .into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    /// `ssb-game` repeats the pack's slot numbering and the thrown
    /// figatree lengths, because Layer A must not depend on `ssb-rom`. This
    /// pins both copies to the generated table.
    #[test]
    fn grab_slots_and_thrown_lengths_match_the_anim_table() {
        use ssb_game::fighter::FighterKind;
        use ssb_game::status::{AnyStatus, DonkeyStatus, SamusStatus, Status};
        let common = [
            Status::Catch,
            Status::CatchPull,
            Status::ThrowF,
            Status::ThrowB,
            Status::CapturePulled,
            Status::ThrownDonkeyF,
            Status::ThrownMarioBStart,
            Status::ThrownFoxFStart,
            Status::Shouldered,
            Status::ThrownMarioB,
            Status::ThrownCommon,
            Status::ThrownFoxF,
            Status::ThrownFoxB,
        ];
        for (i, status) in common.iter().enumerate() {
            let slot = AnyStatus::Common(*status).anim_slot();
            assert_eq!(slot, ssb_rom::anim::SLOT_CATCH + i, "{status:?}");
        }
        assert_eq!(
            AnyStatus::Donkey(DonkeyStatus::ThrowFWait).anim_slot(),
            ssb_rom::anim::SLOT_DONKEY_THROWF_WAIT
        );
        assert_eq!(
            AnyStatus::Donkey(DonkeyStatus::ThrowAirFF).anim_slot(),
            ssb_rom::anim::SLOT_DONKEY_THROWF_WAIT + 10
        );
        assert_eq!(
            AnyStatus::Samus(SamusStatus::SpecialNStart).anim_slot(),
            ssb_rom::anim::SLOT_SAMUS_SPECIAL_N_START
        );
        assert_eq!(
            AnyStatus::Samus(SamusStatus::SpecialAirLw).anim_slot(),
            ssb_rom::anim::SLOT_SAMUS_SPECIAL_N_START + 8
        );
        for (row, kind) in [
            FighterKind::Mario,
            FighterKind::Fox,
            FighterKind::Donkey,
            FighterKind::Samus,
            FighterKind::Luigi,
        ]
        .into_iter()
        .enumerate()
        {
            for status in &common[5..] {
                let slot = AnyStatus::Common(*status).anim_slot();
                let want = ssb_rom::anim::EXPECTED_FRAMES[row][slot];
                let got = ssb_game::grab::thrown_length(kind, *status).unwrap_or(0.0);
                assert_eq!(got, f32::from(want), "{kind:?} {status:?}");
            }
        }
    }

    #[test]
    fn grab_motion_flags_distinguish_model_parts_from_runtime_joints() {
        use ssb_rom::anim::{LEADING_RUNTIME_JOINT, SLOT_CATCH};

        // Catch, CatchPull and ThrowF have no leading runtime joint for these
        // fighters, whereas CapturePulled does.
        for kind in 0..5 {
            for slot in SLOT_CATCH..SLOT_CATCH + 3 {
                assert!(
                    !LEADING_RUNTIME_JOINT[kind][slot],
                    "kind {kind} slot {slot}"
                );
            }
            assert!(LEADING_RUNTIME_JOINT[kind][SLOT_CATCH + 4]);
        }
    }

    #[test]
    fn camera_head1_graphs_restore_xlu_mode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut total = 0;
        let mut mode_writers = 0;
        for (file_id, graphs) in &loaded.graphs {
            let Some(file) = loaded.files.get(*file_id as usize).and_then(Option::as_ref) else {
                continue;
            };
            let resolver = ssb_rom::scene::DlResolver::new(file);
            for graph in graphs {
                let plan = super::plan_draw_order(graph, &resolver);
                let heads: Vec<_> = plan.iter().filter(|p| p.list_id == Some(1)).collect();
                if heads.is_empty() {
                    continue;
                }
                total += 1;
                let mut own_modes = Vec::new();
                for p in heads {
                    let cmds = file
                        .data
                        .get(p.dl as usize..)
                        .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                        .unwrap_or_default();
                    for c in &cmds {
                        if let ssb_rom::dl::Cmd::SetOtherModeL { shift: 3, data, .. } = c {
                            own_modes.push(*data);
                        }
                    }
                }
                if !own_modes.is_empty() {
                    mode_writers += 1;
                    assert_eq!(
                        own_modes,
                        [0x0C18_49D8, 0x0050_49D8],
                        "file {file_id} graph {:X}",
                        graph.offset
                    );
                }
            }
        }
        assert_eq!(total, 103);
        assert_eq!(mode_writers, 3);
    }

    #[test]
    fn holdout_rejects_a_training_only_saddle_improvement() {
        use ssb_rom::{filter_compensation as fc, texture::Rgba8};

        let mut original = Rgba8::new(2, 2);
        for (i, c) in [0, 255, 255, 0].into_iter().enumerate() {
            original.put(i, [c, c, c, 255]);
        }
        let mut candidate = original.clone();
        candidate.put(3, [255, 255, 255, 255]);
        let train = [[16, 16]];
        let validation = [[31, 31]];
        let baseline_train = fc::measure_samples_policy(
            &original,
            &original,
            true,
            true,
            &train,
            fc::AlphaPolicy::Opaque,
        );
        let candidate_train = fc::measure_samples_policy(
            &original,
            &candidate,
            true,
            true,
            &train,
            fc::AlphaPolicy::Opaque,
        );
        assert!(candidate_train.squared_error < baseline_train.squared_error);
        let baseline_validation = fc::measure_samples_policy(
            &original,
            &original,
            true,
            true,
            &validation,
            fc::AlphaPolicy::Opaque,
        );
        let candidate_validation = fc::measure_samples_policy(
            &original,
            &candidate,
            true,
            true,
            &validation,
            fc::AlphaPolicy::Opaque,
        );
        assert!(!super::validation_preserved(
            baseline_validation,
            candidate_validation,
            fc::AlphaPolicy::Opaque,
        ));
        let empty = fc::measure_samples_policy(
            &original,
            &original,
            true,
            true,
            &[],
            fc::AlphaPolicy::Opaque,
        );
        assert!(!super::validation_preserved(
            empty,
            empty,
            fc::AlphaPolicy::Opaque
        ));
    }
    use super::{
        normal_transform_equivalent, palette_bank_offset, verify_texgen, TexgenCensus, TileState,
        DIRECT_MANAGER_EFFECT_ASSETS, DIRECT_MANAGER_EFFECT_MOBJ_PAIRS,
        EF_COMMON_EFFECTS2_MOBJ_PAIRS, MANAGER_EFFECT_ASSETS,
    };
    use std::collections::BTreeSet;

    /// `PLAN.md` R2.1/T10: `verify_texgen`'s failure branches, exercised
    /// synthetically so they're host-testable without `SSB64_ROM` -- the
    /// real-archive walk (`texgen_addressing_census_against_real_archive_materials`,
    /// below) already proves the *current* ROM passes every one of these; this
    /// proves the checks themselves actually fire (`AGENTS.md`'s "test the
    /// test by breaking the code": a check that can never fail is not
    /// evidence).
    #[test]
    fn verify_texgen_passes_on_a_clean_census_and_fails_on_each_named_violation() {
        let clean = TexgenCensus::default();
        assert!(
            verify_texgen(&clean).is_empty(),
            "a default (empty) census should have no correctness-critical violations"
        );

        let mode_mismatch = TexgenCensus {
            load_draw_mode_mismatch: 1,
            ..TexgenCensus::default()
        };
        assert!(!verify_texgen(&mode_mismatch).is_empty());

        let scale_mismatch = TexgenCensus {
            load_draw_scale_mismatch: 1,
            ..TexgenCensus::default()
        };
        assert!(!verify_texgen(&scale_mismatch).is_empty());

        let mut nonzero_shift = TexgenCensus::default();
        nonzero_shift.texgen_tiles.insert(
            TileState {
                shift: (1, 0),
                ..TileState::default()
            },
            1,
        );
        assert!(!verify_texgen(&nonzero_shift).is_empty());
    }

    /// `PLAN.md` R2.1/T10 (RE-239): archive-wide check for the mapping-
    /// transition test minimum's `textured→untextured→texgen` case. Walks
    /// every real file's converted meshes (`file_meshes`, the same path
    /// `pack` uses) and counts texgen-mode primitives with no bound texture.
    /// Real ROM measured `0` of `202` real texgen primitives lack a texture
    /// -- this content never actually reaches that combination, so the
    /// dedicated transition test lives as a synthetic case in
    /// `mesh.rs`'s `a_texgen_primitive_after_an_untextured_primitive_has_no_stale_texture`,
    /// not a real-archive regression here (same "not seen in the real
    /// archive" shape as T7's `texgen_addressing_reference_cases_for_material_combinations_not_seen_in_the_real_archive`).
    #[test]
    fn no_real_texgen_primitive_is_missing_a_bound_texture() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut texgen_prims = 0usize;
        let mut missing_texture = 0usize;
        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            for mesh in super::file_meshes(&loaded, file) {
                for p in &mesh.primitives {
                    if p.material.texture_gen != ssb_rom::mesh::TextureGen::None {
                        texgen_prims += 1;
                        if p.material.texture.is_none() {
                            missing_texture += 1;
                        }
                    }
                }
            }
        }
        println!(
            "R2.1/T10 texgen-without-texture census: {missing_texture} of {texgen_prims} real texgen primitives lack a bound texture"
        );
        assert!(
            texgen_prims > 0,
            "archive-wide walk found no texgen primitives"
        );
        assert_eq!(
            missing_texture, 0,
            "a real texgen primitive with no bound texture now exists: promote the synthetic \
             transition test's assumption to a real-archive one and check the new content \
             renders correctly untextured"
        );
    }

    /// RE-253: before trusting `format`/`size`/`palette_entries`/`palette`
    /// (the CI4 bank) as safe to leave out of `TexKey`, measured whether a
    /// fixed `(data_file, data_offset)`/`(palette_file, palette_offset)`
    /// pair ever decodes two different ways archive-wide. It does not --
    /// this census measured 1 real format/size conflict and 46 real
    /// palette-shape conflicts, so all four fields are in `TexKey` (its own
    /// doc comment) regardless. Kept as a permanent regression census, the
    /// same shape RE-240's `census_lit_primitives_with_a_colour_baking_
    /// branch` already uses: these exact counts are what a fixed ROM
    /// produces today, and a future change to how tiles/palettes get
    /// resolved that silently made them common again would be the kind of
    /// regression worth re-measuring, not just re-fixing.
    #[test]
    fn texture_key_fields_never_vary_for_a_fixed_data_and_palette_location() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut data_shape: std::collections::BTreeMap<
            (u32, u32),
            (ssb_rom::texture::Format, ssb_rom::texture::BitSize),
        > = std::collections::BTreeMap::new();
        let mut palette_shape: std::collections::BTreeMap<(u32, u32), (u16, u8)> =
            std::collections::BTreeMap::new();
        let mut data_conflicts = 0usize;
        let mut palette_conflicts = 0usize;
        let mut textures_seen = 0usize;
        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            for mesh in super::file_meshes(&loaded, file) {
                for p in &mesh.primitives {
                    let Some(t) = p.material.texture else {
                        continue;
                    };
                    if t.framebuffer {
                        continue;
                    }
                    textures_seen += 1;
                    let data_key = (t.data_file.map_or(id, u32::from), t.data_offset);
                    let shape = (t.format, t.size);
                    match data_shape.entry(data_key) {
                        std::collections::btree_map::Entry::Vacant(e) => {
                            e.insert(shape);
                        }
                        std::collections::btree_map::Entry::Occupied(e) => {
                            if *e.get() != shape {
                                data_conflicts += 1;
                            }
                        }
                    }
                    if let Some(off) = t.palette_offset {
                        let palette_key = (t.palette_file.map_or(id, u32::from), off);
                        let shape = (t.palette_entries, t.palette);
                        match palette_shape.entry(palette_key) {
                            std::collections::btree_map::Entry::Vacant(e) => {
                                e.insert(shape);
                            }
                            std::collections::btree_map::Entry::Occupied(e) => {
                                if *e.get() != shape {
                                    palette_conflicts += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        println!(
            "RE-253 texture-key-field census: {textures_seen} real texture bindings, \
             {} distinct data locations, {} distinct palette locations, \
             {data_conflicts} format/size conflicts, {palette_conflicts} palette-shape conflicts",
            data_shape.len(),
            palette_shape.len()
        );
        assert!(textures_seen > 0, "archive-wide walk found no textures");
        // Both fields are already in `TexKey` (its own doc comment), so a
        // nonzero count here does not mean textures render wrong -- it means
        // this census's own baseline moved, worth a fresh look either way.
        assert_eq!(
            data_conflicts, 1,
            "format/size conflict count for a fixed (data_file, data_offset) changed from RE-253's \
             measured baseline of 1"
        );
        assert_eq!(
            palette_conflicts, 46,
            "palette_entries/bank conflict count for a fixed (palette_file, palette_offset) changed \
             from RE-253's measured baseline of 46"
        );
    }

    /// `R2.2`/C1 (RE-240): measures how many real primitives are both `lit`
    /// (vertex bytes are a packed normal, per `mesh.rs`'s `push_vertex` doc
    /// comment) and also carry one of `push_vertex`'s three colour-baking
    /// branches (`prim_color`, `texture_blend`, `flat_color`). Before RE-240
    /// each of these unconditionally mutated `v.rgba` with no `material.lit`
    /// gate, corrupting the normal for exactly the primitives counted here
    /// (243/34/2 measured); kept as a permanent regression census, not a
    /// temporary probe, since a future change reintroducing an unconditional
    /// bake would otherwise need this same measurement redone to notice.
    ///
    /// This can't assert a flat zero "not a plausible normal" count: some
    /// real vertices are legitimately shared between a lit primitive and an
    /// earlier unlit one (`push_vertex`'s own dedup, RE-103's precedent for
    /// exactly this ambiguity), which already measures a nonzero
    /// `looks_like_unit_normal` failure rate with *no* baking branch
    /// involved at all (808 vertices, unaffected by this fix either way --
    /// confirmed by temporarily disabling RE-240's own `lit` gate and
    /// re-running this census: the *with-branch* count jumped from 60 to
    /// 6,666 while the *without-branch* baseline stayed at exactly 808).
    /// So this compares rates instead: the with-branch rate must stay within
    /// a generous multiple of the unrelated without-branch baseline, rather
    /// than assuming either can be exactly zero.
    #[test]
    fn census_lit_primitives_with_a_colour_baking_branch() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut lit_prims = 0usize;
        let mut lit_prim_color = 0usize;
        let mut lit_texture_blend = 0usize;
        let mut lit_flat_color = 0usize;
        let mut with_branch_checked = 0usize;
        let mut with_branch_not_normal = 0usize;
        let mut without_branch_checked = 0usize;
        let mut without_branch_not_normal = 0usize;
        let mut affected_files: BTreeSet<u32> = BTreeSet::new();
        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            for mesh in super::file_meshes(&loaded, file) {
                for p in &mesh.primitives {
                    if !p.material.lit {
                        continue;
                    }
                    lit_prims += 1;
                    let has_baking_branch = p.material.prim_color.is_some()
                        || p.material.texture_blend.is_some()
                        || p.material.flat_color.is_some();
                    if has_baking_branch {
                        affected_files.insert(id);
                    }
                    if p.material.prim_color.is_some() {
                        lit_prim_color += 1;
                    }
                    if p.material.texture_blend.is_some() {
                        lit_texture_blend += 1;
                    }
                    if p.material.flat_color.is_some() {
                        lit_flat_color += 1;
                    }
                    for &i in &p.indices {
                        let Some(v) = mesh.vertices.get(i as usize) else {
                            continue;
                        };
                        let normal_looking = ssb_rom::pack::looks_like_unit_normal(v.rgba);
                        if has_baking_branch {
                            with_branch_checked += 1;
                            if !normal_looking {
                                with_branch_not_normal += 1;
                            }
                        } else {
                            without_branch_checked += 1;
                            if !normal_looking {
                                without_branch_not_normal += 1;
                            }
                        }
                    }
                }
            }
        }
        println!(
            "R2.2/C1 lit+colour-bake census: {lit_prims} lit primitives; \
             prim_color={lit_prim_color} texture_blend={lit_texture_blend} \
             flat_color={lit_flat_color}; not-normal-looking vertices: \
             with_branch={with_branch_not_normal}/{with_branch_checked} \
             without_branch={without_branch_not_normal}/{without_branch_checked}"
        );
        // None of these match a `FIGHTER_FILES` entry directly -- those name
        // each fighter's `FTAttributes` file, not the separate mesh/costume
        // files their models actually live in (e.g. file 296 here is Mario's
        // own hat, RE-106's original example, in a costume file distinct
        // from `FIGHTER_FILES`'s `Mario` entry at file 203).
        let ids: Vec<String> = affected_files.iter().map(u32::to_string).collect();
        println!("R2.2/C1 affected archive files: {}", ids.join(", "));
        let with_branch_rate = with_branch_not_normal as f64 / with_branch_checked as f64;
        let baseline_rate = without_branch_not_normal as f64 / without_branch_checked as f64;
        assert!(
            with_branch_rate <= baseline_rate * 5.0 + 0.01,
            "with-branch not-normal-looking rate ({with_branch_rate:.4}) is far above the \
             unrelated baseline ({baseline_rate:.4}) -- a colour-baking branch is corrupting \
             lit vertices again (RE-240 regression)"
        );
    }

    /// `R2.2`/C2 (RE-241): measures how often a vertex's own load-time `lit`
    /// (`MeshVertex::lit`, captured at `G_VTX`) disagrees with the
    /// primitive-level, triangle-time `material.lit` the pre-RE-241 pipeline
    /// used in its place. A disagreement is exactly the "vertex meaning
    /// depends on triangle-time state" bug RE-241 fixed: a display list that
    /// toggles `G_LIGHTING` between loading a cache slot and drawing a
    /// triangle that reuses it. Kept as a permanent regression census, like
    /// RE-240's own -- a future change that goes back to reading
    /// `self.material.lit` in `push_vertex` (mesh.rs) or `p.material.lit` in
    /// `add_mesh` (pack.rs) would need this same measurement redone to
    /// notice it silently reintroduced the bug.
    #[test]
    fn census_g_vtx_vs_triangle_time_lighting_state() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut vertex_refs_checked = 0usize;
        let mut disagreements = 0usize;
        let mut affected_files: BTreeSet<u32> = BTreeSet::new();
        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            for mesh in super::file_meshes(&loaded, file) {
                for p in &mesh.primitives {
                    for &i in &p.indices {
                        let Some(v) = mesh.vertices.get(i as usize) else {
                            continue;
                        };
                        vertex_refs_checked += 1;
                        if v.lit != p.material.lit {
                            disagreements += 1;
                            affected_files.insert(id);
                        }
                    }
                }
            }
        }
        println!(
            "R2.2/C2 load-time-vs-triangle-time lighting census: \
             {disagreements}/{vertex_refs_checked} triangle-corner vertex references \
             disagree between G_VTX load time and triangle draw time"
        );
        let ids: Vec<String> = affected_files.iter().map(u32::to_string).collect();
        println!("R2.2/C2 affected archive files: {}", ids.join(", "));
    }

    /// `R2.2`/C2 (RE-243): measures how many real vertices `mesh::
    /// convert_sequence`'s `initial_lit` parameter actually changes, by
    /// converting every one of the 27 fighters' two `common_parts` skeleton
    /// graphs twice -- once with `initial_lit: true` (what `pack`/
    /// `file_meshes` now do) and once with `false` (the pre-fix behaviour)
    /// -- and counting `MeshVertex::lit` disagreements.
    ///
    /// **Result: 0 of 0 graphs measured any difference.** RE-105 already
    /// found that a real `G_MW_LIGHTCOL` (`gSPLightColor`) command is an
    /// unambiguous, ROM-verified signal that a segment is about to draw lit
    /// geometry, and every one of these graphs' node lists that draws any
    /// vertex at all turns out to carry its own `G_MW_LIGHTCOL` ahead of its
    /// first `G_VTX` -- so `mesh.rs`'s existing `Cmd::MoveWord` handler
    /// (`state.material.lit = true`, RE-105) already resolves every one of
    /// these vertices correctly with no external seed needed. The
    /// `ftDisplayMainProcDisplay` external `G_LIGHTING` RE-241 traced is real
    /// hardware behaviour and the seed is the correct, evidenced way to
    /// model it (still exercised by `mesh.rs`'s own unit tests,
    /// `convert_sequence_initial_lit_true_resolves_lit_with_no_in_list_g_lighting`),
    /// but it is redundant for every fighter this archive actually ships --
    /// this is a genuine measured null result, not a wiring bug, and it is
    /// kept as a permanent census so a future change to either signal has a
    /// concrete number to check against.
    #[test]
    fn census_initial_lit_seed_measured_impact_on_skeleton_graphs() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut graphs_checked: BTreeSet<(u32, u32)> = BTreeSet::new();
        let mut vertices_checked = 0usize;
        let mut changed = 0usize;
        for entry in ssb_rom::fighter::FIGHTER_FILES.iter() {
            let Some(main) = loaded.files[entry.file as usize].as_ref() else {
                continue;
            };
            for part in ssb_rom::fighter::common_parts(main, *entry)
                .into_iter()
                .flatten()
            {
                if !graphs_checked.insert((part.model_file, part.graph)) {
                    continue; // several fighters share one model file/graph
                }
                let Some(file) = loaded.files[part.model_file as usize].as_ref() else {
                    continue;
                };
                let Some(g) = loaded
                    .graphs
                    .get(&part.model_file)
                    .and_then(|gs| gs.iter().find(|g| g.offset == part.graph))
                else {
                    continue;
                };
                let resolver = ssb_rom::scene::DlResolver::new(file);
                let plan = super::plan_draw_order(g, &resolver);
                let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                    .iter()
                    .map(|p| {
                        file.data
                            .get(p.dl as usize..)
                            .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                            .unwrap_or_default()
                    })
                    .collect();
                let materials = loaded.materials(file, g);
                let items: Vec<ssb_rom::mesh::SequenceItem> = plan
                    .iter()
                    .zip(&decoded)
                    .map(|(p, cmds)| ssb_rom::mesh::SequenceItem {
                        cmds,
                        world: p.world,
                        mobjs: &materials[p.node],
                        mat_anims: &[],
                        depth_seed: None,
                        stream: 0,
                    })
                    .collect();
                let seeded = ssb_rom::mesh::convert_sequence(
                    &items,
                    ssb_rom::mesh::Source::of(file),
                    ssb_rom::mesh::InitialMaterial::FIGHTER_EXTERNAL,
                );
                let unseeded = ssb_rom::mesh::convert_sequence(
                    &items,
                    ssb_rom::mesh::Source::of(file),
                    ssb_rom::mesh::InitialMaterial::default(),
                );
                for (a, b) in seeded.iter().zip(&unseeded) {
                    let (Ok(a), Ok(b)) = (a, b) else { continue };
                    for (va, vb) in a.vertices.iter().zip(&b.vertices) {
                        vertices_checked += 1;
                        if va.lit != vb.lit {
                            changed += 1;
                        }
                    }
                }
            }
        }
        println!(
            "R2.2/C2 initial_lit seed measured impact: {changed}/{vertices_checked} vertices \
             changed lit state across {} distinct skeleton graphs",
            graphs_checked.len()
        );
        assert!(
            vertices_checked > 0,
            "no skeleton-graph vertices were checked at all -- fighter_skeleton_graphs or \
             common_parts likely regressed"
        );
    }

    /// `R2.2`/C2 (RE-243): the same "external, per-object `G_LIGHTING`" gap
    /// RE-021 named (`looks_like_unit_normal` covering 36,356 of ~37,000
    /// unlit vertices archive-wide), requantified through the packer's own
    /// real conversion path (`file_meshes`) now that fighter skeleton graphs
    /// seed `initial_lit`. Split by whether the vertex came from one of
    /// those seeded graphs, so a future change to the seeding logic has a
    /// concrete regression to compare against. See the sibling
    /// `census_initial_lit_seed_measured_impact_on_skeleton_graphs` for why
    /// the skeleton-graph number here is unchanged by the seed itself
    /// (RE-105's `G_MW_LIGHTCOL` already covers it) -- this fallback usage
    /// comes from elsewhere: vertices legitimately shared between a lit and
    /// an unlit primitive (RE-240's own dedup precedent), or non-fighter
    /// geometry that never carries `G_LIGHTING` at all (RE-241).
    #[test]
    fn census_looks_like_unit_normal_fallback_after_skeleton_lighting_seed() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let skeleton_graphs = super::fighter_skeleton_graphs(&loaded);
        let mut skeleton_unlit_verts = 0usize;
        let mut skeleton_still_normal_like = 0usize;
        let mut other_unlit_verts = 0usize;
        let mut other_still_normal_like = 0usize;
        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            let graphs: &[ssb_rom::scene::SceneGraph] =
                loaded.graphs.get(&id).map_or(&[], Vec::as_slice);
            let on_a_skeleton = graphs
                .iter()
                .any(|g| skeleton_graphs.contains(&(id, g.offset)));
            for mesh in super::file_meshes(&loaded, file) {
                for v in &mesh.vertices {
                    if v.lit {
                        continue;
                    }
                    let normal_like = ssb_rom::pack::looks_like_unit_normal(v.rgba);
                    if on_a_skeleton {
                        skeleton_unlit_verts += 1;
                        skeleton_still_normal_like += normal_like as usize;
                    } else {
                        other_unlit_verts += 1;
                        other_still_normal_like += normal_like as usize;
                    }
                }
            }
        }
        println!(
            "R2.2/C2 looks_like_unit_normal fallback after skeleton-lighting seed: \
             skeleton graphs {skeleton_still_normal_like}/{skeleton_unlit_verts} unlit \
             vertices still look like a normal; other graphs \
             {other_still_normal_like}/{other_unlit_verts}"
        );
    }

    /// `R2.2`/C3 (RE-244): censuses every real primitive's independent
    /// `Z_CMP`/`Z_UPD`/`ZMODE` render-mode bits against `G_ZBUFFER`
    /// (`MeshMaterial::z_buffer`), the geometry-mode bit the pre-C3 PSP
    /// renderer used alone as its only depth-test signal
    /// (`psp-asset-viewer/src/meshdraw.rs`'s `apply_material`, RE-068). Answers whether
    /// that single-flag heuristic already matched the real independent RDP
    /// bits archive-wide, or whether primitives exist where they diverge --
    /// the concrete number `R2.2`/C3's data model change needs before this
    /// project can claim the two "corrective" cases (depth-test-without-
    /// write translucency, and any primitive drawn with `Z_CMP`/`Z_UPD` off
    /// despite `G_ZBUFFER`) are real, not theoretical.
    ///
    /// **Result, after `InitialMaterial::FIGHTER_EXTERNAL` seeding**: 5872
    /// primitives; `z_buffer`=5792, `depth_test`=1324, `depth_write`=1308;
    /// `depth_test != depth_write` in 16 (real depth-test-without-write
    /// translucency, `ZMODE` `XLU`); `z_buffer != depth_test` in 4468. The
    /// fighter-skeleton seed measurably raised `depth_test`/`depth_write`
    /// from 592/576 (see `census_depth_seed_measured_impact_on_skeleton_
    /// graphs` below) but the majority of `z_buffer`-true primitives are
    /// still not fighter-skeleton geometry at all -- stage, effect, and
    /// other non-fighter object categories likely have their own external
    /// per-object `G_SETRENDERMODE` wrapper this project has not yet
    /// identified, the same open shape RE-021/RE-241's "other graphs"
    /// `looks_like_unit_normal` remainder already has for lighting. Until
    /// that wrapper (or an in-list-vs-inherited distinction) is found,
    /// `z_buffer` stays the PSP-side depth-test signal (RE-068, already
    /// device-validated); switching to raw `depth_test` now would regress
    /// the ~4468 primitives this census has not yet explained.
    #[test]
    fn census_independent_depth_state_vs_z_buffer_geometry_bit() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut prims_checked = 0usize;
        let mut z_buffer_true = 0usize;
        let mut depth_test_true = 0usize;
        let mut depth_write_true = 0usize;
        let mut depth_test_write_diverge = 0usize;
        let mut z_buffer_vs_depth_test_diverge = 0usize;
        let mut zmode_counts: std::collections::BTreeMap<&'static str, usize> =
            std::collections::BTreeMap::new();
        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            for mesh in super::file_meshes(&loaded, file) {
                for p in &mesh.primitives {
                    prims_checked += 1;
                    let m = &p.material;
                    z_buffer_true += m.z_buffer as usize;
                    depth_test_true += m.depth_test as usize;
                    depth_write_true += m.depth_write as usize;
                    if m.depth_test != m.depth_write {
                        depth_test_write_diverge += 1;
                    }
                    if m.z_buffer != m.depth_test {
                        z_buffer_vs_depth_test_diverge += 1;
                    }
                    let name = match m.depth_mode {
                        ssb_rom::mesh::ZMode::Opaque => "OPA",
                        ssb_rom::mesh::ZMode::Interpenetrating => "INTER",
                        ssb_rom::mesh::ZMode::Translucent => "XLU",
                        ssb_rom::mesh::ZMode::Decal => "DEC",
                    };
                    *zmode_counts.entry(name).or_default() += 1;
                }
            }
        }
        println!(
            "R2.2/C3 independent depth state census: {prims_checked} primitives; \
             z_buffer(G_ZBUFFER)={z_buffer_true}, depth_test(Z_CMP)={depth_test_true}, \
             depth_write(Z_UPD)={depth_write_true}; depth_test!=depth_write in \
             {depth_test_write_diverge}; z_buffer!=depth_test in \
             {z_buffer_vs_depth_test_diverge}; ZMODE counts {zmode_counts:?}"
        );
        assert!(prims_checked > 0);
    }

    /// `R2.2`/C3 (RE-246): measures how many real primitives
    /// `InitialMaterial::LB_TRANSITION_EXTERNAL`'s depth seed actually flips,
    /// the same shape as `census_depth_seed_measured_impact_on_skeleton_
    /// graphs` and `census_ground_layer1_seed_measured_impact`. This seed's
    /// discovery came from breaking `census_independent_depth_state_vs_
    /// z_buffer_geometry_bit`'s ~4468/3891-primitive gap down by archive
    /// file id: eleven files (the `dLBTransitionDescs` transition scenes,
    /// `ssb_rom::transition::ASSETS`) accounted for roughly 2500 of it, each
    /// diverging on effectively every one of its own primitives -- a much
    /// larger and cleaner cluster than the item/effect files sharing the
    /// remainder, which is why this seed exists but no per-item-type one
    /// does yet.
    #[test]
    fn census_lb_transition_seed_measured_impact() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut prims_checked = 0usize;
        let mut depth_test_changed = 0usize;
        let mut depth_write_changed = 0usize;
        for asset in ssb_rom::transition::ASSETS {
            let Some(file) = loaded
                .files
                .get(asset.file as usize)
                .and_then(Option::as_ref)
            else {
                continue;
            };
            let Some(g) = loaded
                .graphs
                .get(&asset.file)
                .and_then(|gs| gs.iter().find(|g| g.offset == asset.graph))
            else {
                continue;
            };
            let resolver = ssb_rom::scene::DlResolver::new(file);
            let plan = super::plan_draw_order(g, &resolver);
            let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                .iter()
                .map(|p| {
                    file.data
                        .get(p.dl as usize..)
                        .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                        .unwrap_or_default()
                })
                .collect();
            let materials = loaded.materials(file, g);
            let items: Vec<ssb_rom::mesh::SequenceItem> = plan
                .iter()
                .zip(&decoded)
                .map(|(p, cmds)| ssb_rom::mesh::SequenceItem {
                    cmds,
                    world: p.world,
                    mobjs: &materials[p.node],
                    mat_anims: &[],
                    depth_seed: None,
                    stream: 0,
                })
                .collect();
            let seeded = ssb_rom::mesh::convert_sequence(
                &items,
                ssb_rom::mesh::Source::of(file),
                ssb_rom::mesh::InitialMaterial::LB_TRANSITION_EXTERNAL,
            );
            let unseeded = ssb_rom::mesh::convert_sequence(
                &items,
                ssb_rom::mesh::Source::of(file),
                ssb_rom::mesh::InitialMaterial::default(),
            );
            for (a, b) in seeded.iter().zip(&unseeded) {
                let (Ok(a), Ok(b)) = (a, b) else { continue };
                for (pa, pb) in a.primitives.iter().zip(&b.primitives) {
                    prims_checked += 1;
                    if pa.material.depth_test != pb.material.depth_test {
                        depth_test_changed += 1;
                    }
                    if pa.material.depth_write != pb.material.depth_write {
                        depth_write_changed += 1;
                    }
                }
            }
        }
        println!(
            "R2.2/C3 lb transition depth seed measured impact: \
             {depth_test_changed}/{prims_checked} primitives changed depth_test, \
             {depth_write_changed}/{prims_checked} changed depth_write, across \
             {} transition assets",
            ssb_rom::transition::ASSETS.len()
        );
        assert!(prims_checked > 0);
    }

    /// `R2.2`/C3 (RE-244): measures how many real primitives
    /// `InitialMaterial::FIGHTER_EXTERNAL`'s `depth_test`/`depth_write`
    /// seed actually flips, by converting every one of the 27 fighters' two
    /// `common_parts` skeleton graphs both seeded and unseeded and counting
    /// primitive-level disagreements -- the same measurement shape as
    /// `census_initial_lit_seed_measured_impact_on_skeleton_graphs`, but
    /// unlike that one's zero-vertex null result, this seed is not
    /// redundant: most of these graphs' own node lists never issue their
    /// own `G_SETRENDERMODE` at all, so without the seed they would decode
    /// with depth testing and writing both off, contradicting `G_ZBUFFER`
    /// being on for the same primitives.
    #[test]
    fn census_depth_seed_measured_impact_on_skeleton_graphs() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut graphs_checked: BTreeSet<(u32, u32)> = BTreeSet::new();
        let mut prims_checked = 0usize;
        let mut depth_test_changed = 0usize;
        let mut depth_write_changed = 0usize;
        for entry in ssb_rom::fighter::FIGHTER_FILES.iter() {
            let Some(main) = loaded.files[entry.file as usize].as_ref() else {
                continue;
            };
            for part in ssb_rom::fighter::common_parts(main, *entry)
                .into_iter()
                .flatten()
            {
                if !graphs_checked.insert((part.model_file, part.graph)) {
                    continue; // several fighters share one model file/graph
                }
                let Some(file) = loaded.files[part.model_file as usize].as_ref() else {
                    continue;
                };
                let Some(g) = loaded
                    .graphs
                    .get(&part.model_file)
                    .and_then(|gs| gs.iter().find(|g| g.offset == part.graph))
                else {
                    continue;
                };
                let resolver = ssb_rom::scene::DlResolver::new(file);
                let plan = super::plan_draw_order(g, &resolver);
                let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                    .iter()
                    .map(|p| {
                        file.data
                            .get(p.dl as usize..)
                            .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                            .unwrap_or_default()
                    })
                    .collect();
                let materials = loaded.materials(file, g);
                let items: Vec<ssb_rom::mesh::SequenceItem> = plan
                    .iter()
                    .zip(&decoded)
                    .map(|(p, cmds)| ssb_rom::mesh::SequenceItem {
                        cmds,
                        world: p.world,
                        mobjs: &materials[p.node],
                        mat_anims: &[],
                        depth_seed: None,
                        stream: 0,
                    })
                    .collect();
                let seeded = ssb_rom::mesh::convert_sequence(
                    &items,
                    ssb_rom::mesh::Source::of(file),
                    ssb_rom::mesh::InitialMaterial::FIGHTER_EXTERNAL,
                );
                let unseeded = ssb_rom::mesh::convert_sequence(
                    &items,
                    ssb_rom::mesh::Source::of(file),
                    ssb_rom::mesh::InitialMaterial::default(),
                );
                for (a, b) in seeded.iter().zip(&unseeded) {
                    let (Ok(a), Ok(b)) = (a, b) else { continue };
                    for (pa, pb) in a.primitives.iter().zip(&b.primitives) {
                        prims_checked += 1;
                        if pa.material.depth_test != pb.material.depth_test {
                            depth_test_changed += 1;
                        }
                        if pa.material.depth_write != pb.material.depth_write {
                            depth_write_changed += 1;
                        }
                    }
                }
            }
        }
        println!(
            "R2.2/C3 depth seed measured impact: {depth_test_changed}/{prims_checked} \
             primitives changed depth_test, {depth_write_changed}/{prims_checked} changed \
             depth_write, across {} distinct skeleton graphs",
            graphs_checked.len()
        );
        assert!(prims_checked > 0);
    }

    /// `R2.2`/C3 (RE-245): does stage/ground geometry explain the ~4468-
    /// primitive `z_buffer`-vs-`depth_test` gap `census_independent_depth_
    /// state_vs_z_buffer_geometry_bit` leaves after the fighter-skeleton seed?
    ///
    /// Reading `refs/ssb-decomp-re/src/gr/grdisplay.c` directly found every
    /// `grDisplayLayerNPriProcDisplay`/`SecProcDisplay` sets `G_ZBUFFER` and
    /// `gDPSetRenderMode` unconditionally right before walking the layer's own
    /// `DObj` tree -- the same external "wrap-and-walk" shape
    /// `ftDisplayMainProcDisplay` has for fighters (RE-241/RE-244), but here
    /// it varies **by layer index**: layers 0/2/3 clear `G_ZBUFFER` and set a
    /// non-`ZB` render mode (no depth test or write at all), while layer 1
    /// sets `G_ZBUFFER` and `G_RM_AA_ZB_OPA_SURF`/`G_RM_AA_ZB_XLU_SURF` (depth
    /// test on; write on for task head 0, off for head 1 -- real depth-test-
    /// without-write translucency).
    ///
    /// This census decodes each layer's graph the same way `pack`/`file_meshes`
    /// now do -- through `initial_material_for`, which seeds layer 1 with
    /// [`ssb_rom::mesh::InitialMaterial::GROUND_LAYER1_EXTERNAL`] (RE-245),
    /// corrected per item by `ground_layer1_list1_depth_seed` (RE-250) for
    /// whichever of a graph's nodes resolve to `NodeDl::Links` and target
    /// task list 1 -- and tallies `z_buffer`/`depth_test`/`depth_write` per
    /// layer index, plus how many of a graph's nodes resolve to `Links`
    /// (routed to a specific task `list_id` by the node's own `DObjDLLink`
    /// entries) versus `Direct`/`Pair` (always task list 0).
    ///
    /// **Result**: layers 0/2/3 (`z_buffer_true` 297/18/80) still read
    /// `depth_test`/`depth_write` false throughout -- correct, since their own
    /// external wrapper clears `G_ZBUFFER` and sets a non-`ZB` render mode,
    /// already [`ssb_rom::mesh::InitialMaterial::default`]; a handful of their
    /// own nodes re-enable `G_ZBUFFER` (RSP capability) without repeating
    /// `G_SETRENDERMODE`, so `z_buffer` and `depth_test` legitimately diverge
    /// for them, not a bug. Layer 1 (776 primitives) reads `depth_test` true
    /// for all 776 (matching `z_buffer`), but `depth_write` true for only
    /// **668** -- the other 108, from 21 of its 163 `DObjDLLink` entries
    /// targeting task list 1 (`gr`'s translucent pass, `G_RM_AA_ZB_XLU_SURF`),
    /// now correctly read `depth_write` false (RE-250). This closes the
    /// `PlannedList`/`list_id` remainder [`ssb_rom::mesh::InitialMaterial::
    /// GROUND_LAYER1_EXTERNAL`]'s own doc comment left open.
    #[test]
    fn census_ground_layer_depth_state_vs_z_buffer() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let skeleton_graphs = super::fighter_skeleton_graphs(&loaded);
        let ground_graphs = super::ground_layer1_graphs(&loaded);
        let transition_graphs = super::lb_transition_graphs();

        #[derive(Default, Debug)]
        struct LayerStats {
            prims: usize,
            z_buffer_true: usize,
            depth_test_true: usize,
            depth_write_true: usize,
            direct_or_pair_nodes: usize,
            links_nodes: usize,
            links_list0: usize,
            links_list1: usize,
            links_other: usize,
        }

        let mut by_layer: std::collections::BTreeMap<u32, LayerStats> =
            std::collections::BTreeMap::new();
        for stage in &loaded.stages {
            for layer in &stage.layers {
                let (file_id, graph_offset) = layer.graph;
                let Some(file) = loaded.files.get(file_id as usize).and_then(Option::as_ref) else {
                    continue;
                };
                let Some(g) = loaded
                    .graphs
                    .get(&file_id)
                    .and_then(|gs| gs.iter().find(|g| g.offset == graph_offset))
                else {
                    continue;
                };
                let stats = by_layer.entry(layer.index).or_default();

                let resolver = ssb_rom::scene::DlResolver::new(file);
                for node in &g.nodes {
                    let Some(dl) = node.desc.dl else { continue };
                    match resolver.resolve(dl) {
                        ssb_rom::scene::NodeDl::Links(links) => {
                            stats.links_nodes += 1;
                            for link in &links {
                                match link.list_id {
                                    0 => stats.links_list0 += 1,
                                    1 => stats.links_list1 += 1,
                                    _ => stats.links_other += 1,
                                }
                            }
                        }
                        _ => stats.direct_or_pair_nodes += 1,
                    }
                }

                let plan = super::plan_draw_order(g, &resolver);
                let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                    .iter()
                    .map(|p| {
                        file.data
                            .get(p.dl as usize..)
                            .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                            .unwrap_or_default()
                    })
                    .collect();
                let materials = loaded.materials(file, g);
                let initial = super::initial_material_for(
                    &skeleton_graphs,
                    &ground_graphs,
                    &transition_graphs,
                    file_id,
                    graph_offset,
                );
                let items: Vec<ssb_rom::mesh::SequenceItem> = plan
                    .iter()
                    .zip(&decoded)
                    .map(|(p, cmds)| ssb_rom::mesh::SequenceItem {
                        cmds,
                        world: p.world,
                        mobjs: &materials[p.node],
                        mat_anims: &[],
                        depth_seed: super::ground_layer1_list1_depth_seed(initial, p.list_id),
                        stream: u8::from(p.list_id == Some(1)),
                    })
                    .collect();
                let meshes = ssb_rom::mesh::convert_sequence(
                    &items,
                    ssb_rom::mesh::Source::of(file),
                    initial,
                );
                for mesh in meshes.into_iter().flatten() {
                    for p in &mesh.primitives {
                        stats.prims += 1;
                        stats.z_buffer_true += p.material.z_buffer as usize;
                        stats.depth_test_true += p.material.depth_test as usize;
                        stats.depth_write_true += p.material.depth_write as usize;
                    }
                }
            }
        }
        for (index, stats) in &by_layer {
            println!("R2.2/C3 ground layer {index} depth census: {stats:?}");
        }
        assert!(!by_layer.is_empty());
    }

    /// `R2.2`/C3 (RE-245): measures `InitialMaterial::GROUND_LAYER1_EXTERNAL`'s
    /// real impact, the same shape as `census_depth_seed_measured_impact_on_
    /// skeleton_graphs`. **Result**: of 776 render-layer-1 primitives
    /// archive-wide, the seed flips **577 (74%)** from `depth_test`/
    /// `depth_write` false to true -- most of layer 1's own node lists never
    /// repeat `G_SETRENDERMODE`, exactly like the fighter-skeleton case. This
    /// single seed accounts for 577 of RE-244's 4468-primitive archive-wide
    /// gap (12.9%), shrinking it to 3891
    /// (`census_independent_depth_state_vs_z_buffer_geometry_bit`, re-run
    /// after this seed).
    #[test]
    fn census_ground_layer1_seed_measured_impact() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);
        let mut graphs_checked: BTreeSet<(u32, u32)> = BTreeSet::new();
        let mut prims_checked = 0usize;
        let mut depth_test_changed = 0usize;
        let mut depth_write_changed = 0usize;
        for stage in &loaded.stages {
            for layer in stage.layers.iter().filter(|l| l.index == 1) {
                let (file_id, graph_offset) = layer.graph;
                if !graphs_checked.insert(layer.graph) {
                    continue; // several stages can share one layer-1 graph
                }
                let Some(file) = loaded.files.get(file_id as usize).and_then(Option::as_ref) else {
                    continue;
                };
                let Some(g) = loaded
                    .graphs
                    .get(&file_id)
                    .and_then(|gs| gs.iter().find(|g| g.offset == graph_offset))
                else {
                    continue;
                };
                let resolver = ssb_rom::scene::DlResolver::new(file);
                let plan = super::plan_draw_order(g, &resolver);
                let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                    .iter()
                    .map(|p| {
                        file.data
                            .get(p.dl as usize..)
                            .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                            .unwrap_or_default()
                    })
                    .collect();
                let materials = loaded.materials(file, g);
                let items: Vec<ssb_rom::mesh::SequenceItem> = plan
                    .iter()
                    .zip(&decoded)
                    .map(|(p, cmds)| ssb_rom::mesh::SequenceItem {
                        cmds,
                        world: p.world,
                        mobjs: &materials[p.node],
                        mat_anims: &[],
                        depth_seed: None,
                        stream: 0,
                    })
                    .collect();
                let seeded = ssb_rom::mesh::convert_sequence(
                    &items,
                    ssb_rom::mesh::Source::of(file),
                    ssb_rom::mesh::InitialMaterial::GROUND_LAYER1_EXTERNAL,
                );
                let unseeded = ssb_rom::mesh::convert_sequence(
                    &items,
                    ssb_rom::mesh::Source::of(file),
                    ssb_rom::mesh::InitialMaterial::default(),
                );
                for (a, b) in seeded.iter().zip(&unseeded) {
                    let (Ok(a), Ok(b)) = (a, b) else { continue };
                    for (pa, pb) in a.primitives.iter().zip(&b.primitives) {
                        prims_checked += 1;
                        if pa.material.depth_test != pb.material.depth_test {
                            depth_test_changed += 1;
                        }
                        if pa.material.depth_write != pb.material.depth_write {
                            depth_write_changed += 1;
                        }
                    }
                }
            }
        }
        println!(
            "R2.2/C3 ground layer1 depth seed measured impact: \
             {depth_test_changed}/{prims_checked} primitives changed depth_test, \
             {depth_write_changed}/{prims_checked} changed depth_write, across {} \
             distinct layer-1 graphs",
            graphs_checked.len()
        );
        assert!(prims_checked > 0);
    }

    /// `R2.1`/T1 (RE-225): the normal-relevant part of two node transforms is
    /// only their 3x3 linear part, and only up to a positive uniform scale.
    #[test]
    fn normal_transform_equivalence_ignores_translation_and_uniform_scale() {
        use ssb_rom::scene::Mat4;

        let base = Mat4::from_trs([0.0, 0.0, 0.0], [0.3, 0.5, -0.2], [1.0, 1.0, 1.0]);

        let translated = Mat4::from_trs([10.0, -5.0, 2.0], [0.3, 0.5, -0.2], [1.0, 1.0, 1.0]);
        assert!(
            normal_transform_equivalent(&base, &translated),
            "translation-only difference must not affect the normal-relevant transform"
        );

        let same_rotation = Mat4::from_trs([1.0, 2.0, 3.0], [0.3, 0.5, -0.2], [1.0, 1.0, 1.0]);
        assert!(
            normal_transform_equivalent(&base, &same_rotation),
            "identical rotation under any translation must be equivalent"
        );

        let different_rotation = Mat4::from_trs([0.0, 0.0, 0.0], [0.3, 0.5, 0.4], [1.0, 1.0, 1.0]);
        assert!(
            !normal_transform_equivalent(&base, &different_rotation),
            "a genuinely different rotation must not be equivalent"
        );

        let uniform_scale = Mat4::from_trs([0.0, 0.0, 0.0], [0.3, 0.5, -0.2], [2.5, 2.5, 2.5]);
        assert!(
            normal_transform_equivalent(&base, &uniform_scale),
            "a positive uniform scale does not change the generated normal's direction"
        );

        let non_uniform_scale = Mat4::from_trs([0.0, 0.0, 0.0], [0.3, 0.5, -0.2], [2.0, 1.0, 1.0]);
        assert!(
            !normal_transform_equivalent(&base, &non_uniform_scale),
            "a non-uniform scale changes normal direction and must not be treated as equivalent"
        );
    }

    /// `R2.1`/T1 (RE-225): a texgen triangle reusing an earlier-step vertex is
    /// classified by how the load's node/space compares to the draw's, using
    /// the walker directly rather than a full scene graph (`plan_draw_order`
    /// is what actually produces `space`/`world`; this exercises what `tri`
    /// does with them once set).
    #[test]
    fn texgen_reuse_classifies_same_node_cross_list_and_cross_node() {
        use ssb_rom::scene::Mat4;

        let file = ssb_rom::archive::File {
            id: 7,
            data: vec![0u8; 4],
            extern_relocs: Vec::new(),
            intern_relocs: Vec::new(),
        };
        let identity = Mat4::IDENTITY;

        let mut walk = super::TexgenWalk::new();
        walk.geometry_mode = super::GM_TEXTURE_GEN;
        walk.step = 3;
        walk.dl = 0x100;
        walk.space = Some(0);
        walk.world = identity;

        let loaded = |step, dl, space, world| {
            Some(super::VtxLoadState {
                geometry_mode: super::GM_TEXTURE_GEN,
                tex_scale: (0, 0),
                step,
                dl,
                space,
                world,
            })
        };
        // Loaded this step/list: not a reuse across any boundary.
        walk.cache[0] = loaded(3, 0x100, Some(0), identity);
        // Loaded earlier, same node, same list: same-node reuse.
        walk.cache[1] = loaded(1, 0x100, Some(0), identity);
        // Loaded earlier, same node, a different list (nested `Call`):
        // cross-list same-node reuse.
        walk.cache[2] = loaded(2, 0x080, Some(0), identity);

        let mut census = super::TexgenCensus::default();
        walk.tri([0, 1, 2], &file, 0, 0, &mut census, false);
        assert_eq!(census.same_node_reuse, 1);
        assert_eq!(census.cross_list_same_node_reuse, 1);
        assert_eq!(census.cross_node_equivalent_transform_reuse, 0);
        assert_eq!(census.cross_node_differing_transform_reuse, 0);

        // Loaded under a different node whose rotation genuinely differs:
        // cross-node, differing transform.
        let rotated = Mat4::from_trs(
            [0.0, 0.0, 0.0],
            [0.0, std::f32::consts::FRAC_PI_2, 0.0],
            [1.0, 1.0, 1.0],
        );
        walk.cache[1] = loaded(1, 0x100, Some(9), rotated);
        let mut census = super::TexgenCensus::default();
        walk.tri([0, 1, 2], &file, 0, 0, &mut census, false);
        assert_eq!(census.cross_node_differing_transform_reuse, 1);
        assert_eq!(census.cross_node_equivalent_transform_reuse, 0);

        // Loaded under a different node that is only a uniform-scaled copy
        // of the draw's own transform: cross-node, equivalent transform.
        let scaled = Mat4::from_trs([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        walk.cache[1] = loaded(1, 0x100, Some(9), scaled);
        let mut census = super::TexgenCensus::default();
        walk.tri([0, 1, 2], &file, 0, 0, &mut census, false);
        assert_eq!(census.cross_node_equivalent_transform_reuse, 1);
        assert_eq!(census.cross_node_differing_transform_reuse, 0);
    }

    #[test]
    fn common_effects2_pairs_follow_corrected_decomp_layout() {
        assert_eq!(
            EF_COMMON_EFFECTS2_MOBJ_PAIRS,
            &[
                (84, 0x2040, 0x1EA0),
                (84, 0x2760, 0x22B8),
                (84, 0x3398, 0x2F78),
                (84, 0x53E8, 0x4F08),
                (84, 0x6D00, 0x6B40),
            ]
        );
    }

    #[test]
    fn manager_effect_inventory_has_46_unique_graphs() {
        let ordered_keys: Vec<_> = MANAGER_EFFECT_ASSETS
            .iter()
            .map(|asset| (asset.file, asset.graph))
            .collect();
        assert_eq!(ordered_keys, ssb_rom::effect::MANAGER_EFFECT_KEYS);
        assert_eq!(
            ssb_rom::effect::MANAGER_EFFECT_REST_INVISIBLE_KEYS,
            &[(84, 0x6D00), (349, 0x0B90), (353, 0x11C0)]
        );

        let keys: BTreeSet<_> = ordered_keys.iter().copied().collect();
        assert_eq!(MANAGER_EFFECT_ASSETS.len(), 46);
        assert_eq!(keys.len(), MANAGER_EFFECT_ASSETS.len());

        let direct: BTreeSet<_> = DIRECT_MANAGER_EFFECT_ASSETS
            .iter()
            .map(|asset| (asset.file, asset.graph))
            .collect();
        assert_eq!(direct.len(), 12);
        assert!(direct.is_subset(&keys));
        assert!(DIRECT_MANAGER_EFFECT_MOBJ_PAIRS
            .iter()
            .all(|&(file, graph, _)| direct.contains(&(file, graph))));
    }

    /// `PLAN.md` R2.0/P0a: measures `n64_filter::sample_3point` (the RDP's
    /// real 3-point reconstruction) against `n64_filter::sample_bilinear`
    /// (PSP `Linear`'s symmetric four-tap reconstruction) densely across
    /// every unique real bound texture archive-wide, not a synthetic case.
    /// Dedup and decode logic mirrors `texdump` exactly, since that is the
    /// project's own established "measure through the real pipeline, not a
    /// separate heuristic scan" precedent (RE-112).
    #[test]
    fn filter_reconstruction_census_against_real_archive_textures() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);

        let mut seen: BTreeSet<(u32, u32)> = BTreeSet::new();
        let mut textures_censused = 0usize;
        let mut samples_total = 0u64;
        let mut at_least_8 = 0u64;
        let mut at_least_16 = 0u64;
        let mut at_least_32 = 0u64;
        // (max_diff, file, offset, width, height)
        let mut worst: Vec<(u8, u32, u32, u32, u32)> = Vec::new();
        let mut named: std::collections::BTreeMap<(u32, u32), (u8, f64)> = Default::default();

        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            for m in super::file_meshes(&loaded, file) {
                for prim in &m.primitives {
                    let Some(t) = prim.material.texture else {
                        continue;
                    };
                    let texels = super::Texels {
                        home: file,
                        all: &loaded.files,
                    };
                    let home = t.data_file.map_or(id, u32::from);
                    if !seen.insert((home, t.data_offset)) {
                        continue;
                    }
                    if (t.data_offset >> 24) != 0 || t.data_offset == 0 {
                        continue;
                    }

                    let need = ssb_rom::texture::data_len(t.width as u32, t.height as u32, t.size);
                    let Some(src) = texels
                        .bytes(t.data_file)
                        .and_then(|d| d.get(t.data_offset as usize..t.data_offset as usize + need))
                    else {
                        continue;
                    };
                    let tlut: Vec<u16> = match t.palette_offset {
                        Some(off) => {
                            let (off, n) = palette_bank_offset(off, t.palette_entries, t.palette);
                            texels
                                .bytes(t.palette_file)
                                .and_then(|d| d.get(off as usize..off as usize + n * 2))
                                .map(ssb_rom::texture::parse_tlut)
                                .unwrap_or_default()
                        }
                        None => Vec::new(),
                    };
                    let Ok(img) = ssb_rom::texture::decode(
                        src,
                        t.width as u32,
                        t.height as u32,
                        t.format,
                        t.size,
                        (!tlut.is_empty()).then_some(tlut.as_slice()),
                    ) else {
                        continue;
                    };
                    if img.width < 2 || img.height < 2 {
                        // No interior 2x2 texel quad to filter.
                        continue;
                    }

                    textures_censused += 1;
                    let mut tex_max = 0u8;
                    let mut tex_sum = 0u64;
                    let mut tex_count = 0u64;
                    // 4 samples per texel per axis (1/32-texel step 8).
                    let s_max = img.width as i32 * 32;
                    let t_max = img.height as i32 * 32;
                    let mut sq = 0;
                    while sq < s_max {
                        let mut tq = 0;
                        while tq < t_max {
                            let a = ssb_rom::n64_filter::sample_3point(&img, sq, tq);
                            let b = ssb_rom::n64_filter::sample_bilinear(&img, sq, tq);
                            let d = ssb_rom::n64_filter::max_abs_diff(a, b);
                            tex_max = tex_max.max(d);
                            tex_sum += d as u64;
                            tex_count += 1;
                            samples_total += 1;
                            if d >= 8 {
                                at_least_8 += 1;
                            }
                            if d >= 16 {
                                at_least_16 += 1;
                            }
                            if d >= 32 {
                                at_least_32 += 1;
                            }
                            tq += 8;
                        }
                        sq += 8;
                    }
                    let tex_mean = tex_sum as f64 / tex_count as f64;
                    named.insert((home, t.data_offset), (tex_max, tex_mean));
                    worst.push((
                        tex_max,
                        home,
                        t.data_offset,
                        t.width as u32,
                        t.height as u32,
                    ));
                }
            }
        }

        worst.sort_by_key(|a| std::cmp::Reverse(a.0));
        println!(
            "censused {textures_censused} unique real textures, {samples_total} interior sample points"
        );
        println!(
            "samples with max-channel diff >=8/255:  {at_least_8} ({:.4}%)",
            at_least_8 as f64 * 100.0 / samples_total as f64
        );
        println!(
            "samples with max-channel diff >=16/255: {at_least_16} ({:.4}%)",
            at_least_16 as f64 * 100.0 / samples_total as f64
        );
        println!(
            "samples with max-channel diff >=32/255: {at_least_32} ({:.4}%)",
            at_least_32 as f64 * 100.0 / samples_total as f64
        );
        println!("worst 10 textures by max diff:");
        for (max_diff, file, offset, w, h) in worst.iter().take(10) {
            println!("  file {file} offset {offset:#X} {w}x{h}: max diff {max_diff}");
        }
        // RE-081's two named, on-device-relevant Dream Land canopy
        // textures: "highlight" (magnified) and "gradient" (minified).
        if let Some((max_diff, mean)) = named.get(&(103, 0x5F0)) {
            println!(
                "Dream Land canopy highlight (file 103 offset 0x5F0): max diff {max_diff}, mean diff {mean:.3}"
            );
        }
        if let Some((max_diff, mean)) = named.get(&(103, 0xE20)) {
            println!(
                "Dream Land canopy gradient (file 103 offset 0xE20): max diff {max_diff}, mean diff {mean:.3}"
            );
        }

        assert!(textures_censused > 0, "archive-wide walk found no textures");
    }

    /// `PLAN.md` R2.0/P0b/P0c: measures two of the three addressing
    /// questions against every real primitive archive-wide, using
    /// `ssb_rom::n64_addressing`'s hardware reference model (transcribed
    /// from `angrylion-rdp-plus`, not a reference port) and its paired
    /// `psp_lowering_axis` model of the PSP conversion.
    ///
    /// Bullet 1 (mirror+clamp beyond the first period): for every real
    /// `mirror + clamp` (`cms`/`cmt == 3`) render-tile axis with a nonzero
    /// mask, classifies how many mask periods each real primitive's UV range
    /// actually reaches, and compares the hardware model's addressed texel
    /// against the PSP lowering's at both UV extremes. RE-220 measured this
    /// diverging (99/810 axis instances, 12.22%) against the *old* two-period
    /// bake; RE-221 (`R2.0`/P0c) fixed `texture::mirror_extend`/
    /// `psp_lowering_axis` to bake every period the drawn rect spans, and
    /// this census now asserts the divergence count is zero.
    ///
    /// Bullet 2 (`mask == 0`): for every real axis with `mask == 0` and the
    /// `cm` clamp bit clear (where current PSP code applies `Repeat`, but
    /// `angrylion-rdp-plus`'s `clampens = cs || !mask_s` says real hardware
    /// always clamps), measures whether any real primitive's UV range
    /// actually leaves the tile's logical bounds -- the only condition under
    /// which the forced-clamp rule changes anything observable.
    #[test]
    fn tile_addressing_census_against_real_archive_textures() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);

        #[derive(Default)]
        struct MirrorClampStats {
            axis_instances: u64,
            within_first_period: u64,
            within_mirrored_period: u64,
            immediately_beyond: u64,
            multiple_periods_beyond: u64,
            negative: u64,
            hw_psp_diverge: u64,
        }
        #[derive(Default)]
        struct Mask0Stats {
            axis_instances: u64,
            clamp_bit_clear: u64,
            clamp_bit_clear_out_of_bounds_instances: u64,
        }
        #[derive(Default)]
        struct PotPaddingStats {
            non_pot_clamp_axis_instances: u64,
            near_or_beyond_last_logical_texel: u64,
        }
        let mut mc = MirrorClampStats::default();
        let mut m0 = Mask0Stats::default();
        let mut pp = PotPaddingStats::default();
        let mut unique_mirror_clamp_tiles: BTreeSet<(u32, u32, u8, u8, bool, bool)> =
            BTreeSet::new();
        let mut unique_mask0_clear_tiles: BTreeSet<(u32, u32, bool, bool)> = BTreeSet::new();
        let mut unique_non_pot_clamp_tiles: BTreeSet<(u32, u32, usize, u32)> = BTreeSet::new();
        let mut primitives_examined = 0u64;

        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            for m in super::file_meshes(&loaded, file) {
                for prim in &m.primitives {
                    let Some(t) = prim.material.texture else {
                        continue;
                    };
                    // Texgen primitives (regular or linear) don't read
                    // `MeshVertex::uv` as an authored coordinate at all --
                    // regular texgen is generated live by the GE from the
                    // vertex normal, and linear texgen's CPU-generated
                    // replacement is computed at pack time, not here. This
                    // census is authored-UV-scoped (`PLAN.md` R2.0/P0b);
                    // texgen addressing is `R2.1`/T7's job, consuming this
                    // same reference model with its own scale/origin wiring.
                    if t.framebuffer
                        || prim.indices.is_empty()
                        || prim.material.texture_gen != ssb_rom::mesh::TextureGen::None
                    {
                        continue;
                    }
                    primitives_examined += 1;
                    let home = t.data_file.map_or(id, u32::from);

                    for (mask, mirror, clamp_bit, origin_q2, drawn, axis_idx) in [
                        (
                            t.mask_s,
                            t.mirror_s,
                            t.clamp_s,
                            t.origin_s as i32,
                            t.drawn_width,
                            0usize,
                        ),
                        (
                            t.mask_t,
                            t.mirror_t,
                            t.clamp_t,
                            t.origin_t as i32,
                            t.drawn_height,
                            1usize,
                        ),
                    ] {
                        let coords: Vec<i32> = prim
                            .indices
                            .iter()
                            .map(|&i| m.vertices[i as usize].uv[axis_idx] as i32)
                            .collect();
                        let (Some(&min_c), Some(&max_c)) =
                            (coords.iter().min(), coords.iter().max())
                        else {
                            continue;
                        };

                        if mirror && clamp_bit && mask > 0 {
                            unique_mirror_clamp_tiles.insert((
                                home,
                                t.data_offset,
                                mask,
                                drawn as u8,
                                mirror,
                                clamp_bit,
                            ));
                            mc.axis_instances += 1;
                            let period = 1u32 << mask;
                            // `clamp_bit` is set, so `mesh.rs` already
                            // subtracted the tile origin at load time
                            // (RE-152) -- feed the reference model an
                            // already-relative origin of 0 to avoid
                            // double-subtracting.
                            let far_edge_q2 = (drawn as i32 - 1) << 2;
                            let model = ssb_rom::n64_addressing::TileAxis {
                                shift: 0,
                                origin_q2: 0,
                                far_edge_q2,
                                mask,
                                mirror,
                                clamp_bit,
                            };
                            let min_tex = min_c.div_euclid(32).div_euclid(period as i32);
                            let max_tex = max_c.div_euclid(32).div_euclid(period as i32);
                            if min_tex < 0 {
                                mc.negative += 1;
                            } else if max_tex >= 3 {
                                mc.multiple_periods_beyond += 1;
                            } else if max_tex >= 2 {
                                mc.immediately_beyond += 1;
                            } else if max_tex >= 1 {
                                mc.within_mirrored_period += 1;
                            } else {
                                mc.within_first_period += 1;
                            }
                            let hw_min = ssb_rom::n64_addressing::address_axis(&model, min_c);
                            let hw_max = ssb_rom::n64_addressing::address_axis(&model, max_c);
                            let drawn_u32 = drawn as u32;
                            let psp_min = ssb_rom::n64_addressing::psp_lowering_axis(
                                min_c, period, drawn_u32, mirror, clamp_bit,
                            );
                            let psp_max = ssb_rom::n64_addressing::psp_lowering_axis(
                                max_c, period, drawn_u32, mirror, clamp_bit,
                            );
                            if hw_min != psp_min || hw_max != psp_max {
                                mc.hw_psp_diverge += 1;
                            }
                        }

                        if mask == 0 {
                            m0.axis_instances += 1;
                            if !clamp_bit {
                                unique_mask0_clear_tiles.insert((
                                    home,
                                    t.data_offset,
                                    mirror,
                                    clamp_bit,
                                ));
                                m0.clamp_bit_clear += 1;
                                // `clamp_bit` is clear here, so `mesh.rs`
                                // only subtracted the origin if this was a
                                // framebuffer binding (excluded above) --
                                // the coordinate is still absolute.
                                let out_of_bounds = [min_c, max_c].into_iter().any(|c| {
                                    let rel = c - (origin_q2 << 3);
                                    let idx = rel.div_euclid(32);
                                    idx < 0 || idx > drawn as i32 - 1
                                });
                                if out_of_bounds {
                                    m0.clamp_bit_clear_out_of_bounds_instances += 1;
                                }
                            }
                        }

                        // Bullet 3: PSP POT-padding vs the N64 logical clamp
                        // boundary. Mirror doubles the packed image to
                        // `2 * (1 << mask)` texels, always already a power of
                        // two -- padding is a real no-op there, so this only
                        // applies to a clamped, *unmirrored* axis whose
                        // logical width the pack step still zero-pads.
                        if clamp_bit && !mirror {
                            // The packed image's own dimension (RE-044
                            // mask-narrowed), not the drawn rect `drawn`
                            // above -- this is what `pack_rgba`/
                            // `pack_indexed` actually pads to a power of two.
                            let axis_dim =
                                if axis_idx == 0 { t.width } else { t.height }.max(1) as u32;
                            let padded = ssb_rom::psp_texture::pad_to_power_of_two(axis_dim);
                            if padded != axis_dim {
                                unique_non_pot_clamp_tiles.insert((
                                    home,
                                    t.data_offset,
                                    axis_idx,
                                    axis_dim,
                                ));
                                pp.non_pot_clamp_axis_instances += 1;
                                // Origin already subtracted (clamp_bit is
                                // set): the last logical texel starts at
                                // `(axis_dim - 1) * 32` in this relative
                                // S10.5 basis. Any real sample at or past
                                // that point has a bilinear neighbour that
                                // reads into the zero-filled padding.
                                let last_texel_floor = (axis_dim as i32 - 1) * 32;
                                if max_c >= last_texel_floor {
                                    pp.near_or_beyond_last_logical_texel += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        println!("primitives examined: {primitives_examined}");
        println!();
        println!("bullet 1 -- mirror+clamp (cms/cmt == 3, mask > 0):");
        println!(
            "  unique (file, offset, mask, drawn width, mirror, clamp) tiles: {}",
            unique_mirror_clamp_tiles.len()
        );
        println!("  axis instances (primitive x axis): {}", mc.axis_instances);
        println!(
            "  UV range within first period:       {}",
            mc.within_first_period
        );
        println!(
            "  UV range reaches mirrored period:   {}",
            mc.within_mirrored_period
        );
        println!(
            "  UV range reaches 1 period beyond that: {}",
            mc.immediately_beyond
        );
        println!(
            "  UV range reaches >=2 periods beyond that: {}",
            mc.multiple_periods_beyond
        );
        println!("  UV range goes negative:             {}", mc.negative);
        println!(
            "  hardware model vs current PSP lowering diverge: {} ({:.4}%)",
            mc.hw_psp_diverge,
            mc.hw_psp_diverge as f64 * 100.0 / mc.axis_instances.max(1) as f64
        );
        println!();
        println!("bullet 2 -- mask == 0:");
        println!("  axis instances: {}", m0.axis_instances);
        println!(
            "  with clamp bit clear (angrylion: still forced-clamped): {}",
            m0.clamp_bit_clear
        );
        println!(
            "  unique (file, offset, mirror, clamp) tiles with clamp bit clear: {}",
            unique_mask0_clear_tiles.len()
        );
        println!(
            "  of those, primitive instances whose UV leaves logical bounds: {}",
            m0.clamp_bit_clear_out_of_bounds_instances
        );
        println!();
        println!("bullet 3 -- PSP POT padding vs N64 logical clamp boundary:");
        println!(
            "  unique (file, offset, axis, logical dim) clamped non-POT axes: {}",
            unique_non_pot_clamp_tiles.len()
        );
        println!(
            "  axis instances (primitive x axis): {}",
            pp.non_pot_clamp_axis_instances
        );
        println!(
            "  of those, samples reaching the last logical texel (bilinear neighbour reads padding): {}",
            pp.near_or_beyond_last_logical_texel
        );

        assert!(
            primitives_examined > 0,
            "archive-wide walk found no textured primitives"
        );
        // RE-221 (`R2.0`/P0c): `texture::mirror_extend`/`psp_lowering_axis`
        // now bake every mask period a mirror+clamp axis's drawn rect spans,
        // not just the first mirrored pair (RE-220's measured gap) -- this
        // must stay zero, or the fix has regressed.
        assert_eq!(
            mc.hw_psp_diverge, 0,
            "hardware and PSP lowering diverge on a real mirror+clamp axis (PLAN.md R2.0/P0c regressed)"
        );
        // RE-220: measured zero `mask == 0` render-tile axes on any real
        // drawn primitive, archive-wide -- `angrylion-rdp-plus`'s forced
        // clamp for `mask == 0` (`clampens = cs || !mask_s`) never actually
        // diverges from the current unconditional `cm`-bit-only PSP lowering
        // on real content, so bullet 2 closes with no fix needed. If this
        // regresses, the forced-clamp rule genuinely needs implementing.
        assert_eq!(
            m0.axis_instances, 0,
            "a real mask == 0 axis now exists: implement forced clamp (PLAN.md R2.0/P0b bullet 2)"
        );
    }

    /// `PLAN.md` R2.0/P1: censuses every real render-tile-0 (`tile ==
    /// RENDER_TILE`, `mesh.rs`'s own constant) `G_SETTILE` command
    /// archive-wide for the five fields `dl.rs`'s `Cmd::SetTile` decodes but
    /// `mesh.rs`'s only consumer discards behind a `..` wildcard
    /// (`mesh.rs:1796-1817`): `palette`, `line`, `tmem`, `shift_s`,
    /// `shift_t`.
    ///
    /// Walks the exact same display-list universe `file_meshes` does -- the
    /// graph-driven `plan_draw_order` roots plus
    /// `scan::find_root_display_lists`'s unclaimed orphans -- following
    /// `Cmd::Call`/`Cmd::Branch` itself (mirroring `mesh.rs`'s own
    /// `MAX_DL_DEPTH`-bounded inlining, `mesh.rs:1710-1745`), the same
    /// pattern `texgen`'s own `TexgenWalk` already uses for this file's
    /// other raw-command censuses, rather than adding permanent
    /// instrumentation fields to `TextureRef`/`mesh::State` for a census
    /// that may find nothing worth keeping (RE-121/RE-122's "temporary,
    /// reverted census" standard).
    #[test]
    fn settile_field_census_against_real_archive_textures() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);

        // Matches `mesh.rs`'s own `MAX_DL_DEPTH` and `RENDER_TILE` constants
        // exactly, so this walker inlines callees the same number of levels
        // deep the real converter does, and looks at the same tile the real
        // converter reads from.
        const CENSUS_MAX_DL_DEPTH: u32 = 18;
        const RENDER_TILE: u8 = 0;

        #[derive(Default)]
        struct Stats {
            tile0_settile_instances: u64,
            ci4_tile0_instances: u64,
            palette_nonzero: u64,
            palette_nonzero_ci4: u64,
            distinct_palette_values: BTreeSet<u8>,
            distinct_line_values: BTreeSet<u16>,
            tmem_nonzero: u64,
            distinct_tmem_values: BTreeSet<u16>,
            shift_s_nonzero: u64,
            shift_t_nonzero: u64,
            distinct_shift_s: BTreeSet<u8>,
            distinct_shift_t: BTreeSet<u8>,
            // (file id, palette bank, most recently loaded TLUT's entry
            // count) for every nonzero-palette CI4 instance -- whether the
            // requested bank actually falls inside the loaded TLUT decides
            // whether ignoring `palette` is a real bug or an inert field.
            palette_nonzero_ci4_detail: Vec<(u32, u8, Option<u16>)>,
        }

        fn walk(
            cmds: &[ssb_rom::dl::Cmd],
            file: &ssb_rom::archive::File,
            depth: u32,
            last_tlut_count: &mut Option<u16>,
            stats: &mut Stats,
        ) {
            use ssb_rom::dl::Cmd;
            for cmd in cmds {
                match cmd {
                    // `tile: 5` is this ROM's own convention for a TLUT load
                    // (RE-instrumented test fixtures throughout `mesh.rs`
                    // consistently use it); state persists across `Call`
                    // boundaries the same way `mesh.rs`'s own
                    // `state.palette_entries` does.
                    Cmd::LoadTlut { count, .. } => {
                        *last_tlut_count = Some(*count);
                    }
                    Cmd::SetTile {
                        tile,
                        format,
                        size,
                        line,
                        tmem,
                        palette,
                        shift_s,
                        shift_t,
                        ..
                    } if *tile == RENDER_TILE => {
                        stats.tile0_settile_instances += 1;
                        stats.distinct_line_values.insert(*line);
                        if *tmem != 0 {
                            stats.tmem_nonzero += 1;
                        }
                        stats.distinct_tmem_values.insert(*tmem);
                        let is_ci4 = ssb_rom::texture::Format::from_raw(*format)
                            == Some(ssb_rom::texture::Format::Ci)
                            && ssb_rom::texture::BitSize::from_raw(*size)
                                == Some(ssb_rom::texture::BitSize::Bits4);
                        if is_ci4 {
                            stats.ci4_tile0_instances += 1;
                        }
                        if *palette != 0 {
                            stats.palette_nonzero += 1;
                            stats.distinct_palette_values.insert(*palette);
                            if is_ci4 {
                                stats.palette_nonzero_ci4 += 1;
                                stats.palette_nonzero_ci4_detail.push((
                                    file.id,
                                    *palette,
                                    *last_tlut_count,
                                ));
                            }
                        }
                        if *shift_s != 0 {
                            stats.shift_s_nonzero += 1;
                            stats.distinct_shift_s.insert(*shift_s);
                        }
                        if *shift_t != 0 {
                            stats.shift_t_nonzero += 1;
                            stats.distinct_shift_t.insert(*shift_t);
                        }
                    }
                    Cmd::Call(addr) | Cmd::Branch(addr) => {
                        if depth >= CENSUS_MAX_DL_DEPTH || addr.segment() != 0 {
                            continue;
                        }
                        let at = addr.0 as usize;
                        let Some(bytes) = file.data.get(at..) else {
                            continue;
                        };
                        let Ok(sub) = ssb_rom::dl::decode_list_at(bytes, at as u32) else {
                            continue;
                        };
                        walk(&sub, file, depth + 1, last_tlut_count, stats);
                    }
                    _ => {}
                }
            }
        }

        let mut stats = Stats::default();
        let mut files_examined = 0u64;

        for id in 0..archive.len() as u32 {
            let Some(file) = loaded.files.get(id as usize).and_then(Option::as_ref) else {
                continue;
            };
            files_examined += 1;

            // Exactly `file_meshes`'s own two-source root discovery, so
            // this walker's scope is the scope the real pack covers -- not
            // a superset or subset.
            let resolver = ssb_rom::scene::DlResolver::new(file);
            let graphs: &[ssb_rom::scene::SceneGraph] =
                loaded.graphs.get(&file.id).map_or(&[], Vec::as_slice);
            let mut claimed = BTreeSet::new();
            for graph in graphs {
                let plan = super::plan_draw_order(graph, &resolver);
                for p in &plan {
                    if p.dl == super::NO_LIST {
                        continue;
                    }
                    claimed.insert(p.dl);
                    if let Some(bytes) = file.data.get(p.dl as usize..) {
                        if let Ok(cmds) = ssb_rom::dl::decode_list_at(bytes, p.dl) {
                            walk(&cmds, file, 0, &mut None, &mut stats);
                        }
                    }
                }
            }
            let all = ssb_rom::scan::find_root_display_lists(file);
            let called: BTreeSet<u32> = all.iter().flat_map(|d| d.referenced_lists()).collect();
            for dl in all
                .iter()
                .filter(|d| !called.contains(&d.offset) && !claimed.contains(&d.offset))
            {
                walk(&dl.commands, file, 0, &mut None, &mut stats);
            }
        }

        println!("files examined: {files_examined}");
        println!(
            "tile-0 G_SETTILE instances: {}",
            stats.tile0_settile_instances
        );
        println!("  CI4 tile-0 instances: {}", stats.ci4_tile0_instances);
        println!(
            "  palette nonzero: {} (distinct values: {:?})",
            stats.palette_nonzero, stats.distinct_palette_values
        );
        println!(
            "  palette nonzero on a CI4 tile: {}",
            stats.palette_nonzero_ci4
        );
        println!(
            "  (file, bank, most recently loaded TLUT entry count): {:?}",
            stats.palette_nonzero_ci4_detail
        );
        println!("  distinct line values: {:?}", stats.distinct_line_values);
        println!(
            "  tmem nonzero: {} (distinct values: {:?})",
            stats.tmem_nonzero, stats.distinct_tmem_values
        );
        println!(
            "  shift_s nonzero: {} (distinct values: {:?})",
            stats.shift_s_nonzero, stats.distinct_shift_s
        );
        println!(
            "  shift_t nonzero: {} (distinct values: {:?})",
            stats.shift_t_nonzero, stats.distinct_shift_t
        );

        assert!(files_examined > 0, "archive-wide walk found no files");
        assert!(
            stats.tile0_settile_instances > 0,
            "archive-wide walk found no tile-0 G_SETTILE commands"
        );
        // RE-223: `tmem` is a TMEM staging address the current converter
        // structurally cannot need -- `convert_texture` reads texel bytes
        // straight from the ROM file at `G_SETTIMG`'s own address, never
        // through TMEM addressing -- and it also measures zero archive-wide.
        // If this regresses, re-examine whether that structural argument
        // still holds before assuming it is still irrelevant.
        assert_eq!(
            stats.tmem_nonzero, 0,
            "a real nonzero tmem now exists on a tile-0 G_SETTILE (PLAN.md R2.0/P1)"
        );
        // RE-223: real content never sets a render tile's shift, so
        // `n64_addressing::TileAxis::shift`'s "always 0 in practice" callers
        // (`crates/ssb-rom/src/n64_addressing.rs`'s own tests, the P0c/P0d
        // censuses) are measured, not assumed. If this regresses, the
        // `tcshift_cycle` pre-scale this project has never wired from real
        // `G_SETTILE` data becomes a real, material gap.
        assert_eq!(
            stats.shift_s_nonzero, 0,
            "a real nonzero shift_s now exists on a tile-0 G_SETTILE (PLAN.md R2.0/P1)"
        );
        assert_eq!(
            stats.shift_t_nonzero, 0,
            "a real nonzero shift_t now exists on a tile-0 G_SETTILE (PLAN.md R2.0/P1)"
        );
        // `palette` is deliberately NOT pinned to zero here: RE-223 found 7
        // real CI4 instances (file 86, `ITCommonObject`) requesting bank 1
        // of a 48-entry loaded TLUT. This walker measures the raw command
        // stream, not `convert_texture`'s resolution of it (RE-223/P2 fixed
        // that separately, `tile0_palette_bank_is_carried_onto_the_texture_reference`
        // in `ssb-rom` and `convert_texture_resolves_the_requested_palette_bank`
        // below) -- a nonzero count here is still expected and correct, not
        // a regression to guard against.
    }

    /// `PLAN.md` R2.1/T6: tile-state and lighting audit for texgen-bound
    /// draws specifically. RE-223 (`R2.0`/P1) already measured `shift_s`/
    /// `shift_t` zero across every real render-tile-0 `G_SETTILE`
    /// archive-wide, but that census covered *all* tile-0 binds, not just
    /// the ones a texgen draw is actually reading under. This reuses the
    /// same `build_texgen_census` walk `texgen()` prints from (RE-225 --
    /// RE-229's own T1-T5 infrastructure) rather than a second heuristic,
    /// and confirms the invariant transfers to the texgen-bound subset
    /// before relying on it. Also reports `G_TEXTURE` scale, tile masks/
    /// shifts/`cm`/origin/dims per texgen mode, and raw `G_LIGHTING` state
    /// at each texgen `G_VTX`, so a future task can see the full addressing
    /// and lighting shape at a glance instead of re-deriving it.
    #[test]
    fn texgen_tile_state_and_lighting_audit() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);

        let (census, graph_lists, discovered_lists) =
            super::build_texgen_census(&archive, &loaded, None, false);

        println!("R2.1/T6 tile-state and lighting audit");
        println!("  graph-planned lists      {graph_lists}");
        println!("  discovered root lists    {discovered_lists}");
        println!("  texgen triangles         {}", census.texgen_triangles);
        println!();
        println!("G_TEXTURE scale in force at a texgen draw, by mode");
        for ((mode, (s, t)), n) in &census.texgen_scales_by_mode {
            println!("  {mode:?}  ({s:#06x}, {t:#06x})  {n}");
        }
        println!();
        println!("tile setup bound by a texgen draw, by mode");
        for ((mode, tile), n) in &census.texgen_tiles_by_mode {
            println!(
                "  {mode:?}  fmt {:?} dims {:?} origin {:?} mask {:?} shift {:?} cm {:?}  {n}",
                tile.fmt, tile.dims, tile.origin, tile.mask, tile.shift, tile.cm
            );
        }
        println!();
        println!("raw G_LIGHTING at a texgen G_VTX, by mode");
        for ((mode, lit), n) in &census.texgen_vtx_lighting {
            println!("  {mode:?}  lit {lit}  {n}");
        }

        assert!(
            census.texgen_triangles > 0,
            "archive-wide walk found no texgen triangles"
        );
        // RE-223 measured this zero for every real tile-0 G_SETTILE,
        // archive-wide. Re-checked here against only the texgen-bound
        // subset, since that is the only subset this project's texgen path
        // actually reads shift from. If this regresses, N64 tile shifting
        // needs implementing before T6 can close (PLAN.md R2.1/T6).
        assert!(
            census
                .texgen_tiles_by_mode
                .keys()
                .all(|(_, tile)| tile.shift == (0, 0)),
            "a texgen-bound tile now has a nonzero shift_s/shift_t (PLAN.md R2.1/T6): implement N64 shifting"
        );
    }

    /// `PLAN.md` R2.1/T7: consumes `R2.0`/P0b's `n64_addressing` reference
    /// model (`ssb_rom::n64_addressing::address_axis`/`psp_lowering_axis`,
    /// already proven against authored UVs by
    /// `tile_addressing_census_against_real_archive_textures`) rather than
    /// building a second one, and checks it against every real
    /// (mode, scale, tile) pairing T6's own `texgen_materials_by_mode`
    /// census measured -- the join `texgen_scales_by_mode`/
    /// `texgen_tiles_by_mode` don't keep, since either map alone loses which
    /// scale belongs to which tile once a mode has more than one of either.
    ///
    /// For each real pairing, sweeps every `i8`-quantized dot product
    /// (-127..=127, the exact granularity a real vertex normal produces,
    /// `psp_texture::texgen_dot`'s own `/127` divisor) through both
    /// `regular_texgen_uv`/`linear_texgen_uv` (the validated PSP-side
    /// formula, RE-228/RE-229) and compares the resulting addressed texel
    /// against `address_axis`'s hardware-model result for the same tile.
    /// RE-230 (T6) measured every real texgen tile clamped on both axes
    /// (`cm` only ever `(2,2)`/`(3,2)`/`(2,3)`); a non-clamp axis is flagged
    /// rather than silently skipped, since `texgen_s10_5_addressed` only
    /// subtracts the tile origin on a clamp axis and this test has not
    /// established the non-clamp+nonzero-origin case is even meaningful for
    /// texgen (`texgen_addressing_reference_cases_for_material_combinations_not_seen_in_the_real_archive`,
    /// below, covers non-clamp addressing synthetically with origin 0).
    ///
    /// RE-231 measured a real, narrow divergence -- 9 of 34 real
    /// clamp-without-mirror axis instances whose mask genuinely narrows
    /// below the tile's drawn rect (period << drawn, RE-044) disagreed with
    /// `address_axis` at exactly one sample, `dot = +1` (the sweep's own
    /// extreme, `n = 127`), never at any other sampled dot. Real hardware
    /// keeps mask-wrapping (periodically repeating) all the way to the
    /// drawn rect's true far edge and only clamps there; the PSP-lowering
    /// model held at the *narrowed* mask period's own last texel instead.
    /// `T7a` fixed `n64_addressing::psp_lowering_axis`'s non-mirror clamp
    /// branch and `texture::mirror_axis_len`/`mirror_fold` (the real bake
    /// `meshdraw::bind_texture` addresses) to fold through every period up
    /// to `drawn` before clamping, matching the mirror+clamp case RE-220/
    /// RE-221 already fixed the same way -- the divergence count is 0
    /// again.
    /// `PLAN.md` R2.1/T10 factored the sweep itself out into
    /// `super::verify_texgen_addressing`, shared with `romtool texgen
    /// --verify`; this test supplies the assertions and diagnostics on top
    /// of that shared walk so the CLI and the test can never silently
    /// disagree about what "addressing agrees with hardware" means.
    #[test]
    fn texgen_addressing_census_against_real_archive_materials() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let (data, info) = super::load_rom(path.as_ref()).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let loaded = super::load_all(&archive);

        let (census, _, _) = super::build_texgen_census(&archive, &loaded, None, false);
        let report = super::verify_texgen_addressing(&census);

        println!("R2.1/T7 texgen addressing census");
        println!(
            "  real (mode, scale, tile) pairings examined: {}",
            report.materials_examined
        );
        println!(
            "  axis instances (pairing x axis):            {}",
            report.axis_instances
        );
        println!(
            "  non-clamp axis instances (unsupported):     {}",
            report.non_clamp_axis_instances
        );
        println!(
            "  diverging axis instances:                   {}",
            report.diverging_axis_instances
        );
        println!(
            "  distinct diverging (mode, scale, dim, mask): {}",
            report.diverging_combos.len()
        );
        println!(
            "  of those, diverging away from the sweep's dot=+1 extreme: {}",
            report.diverging_at_non_extreme_dot
        );

        assert!(
            report.materials_examined > 0,
            "archive-wide walk found no real texgen (mode, scale, tile) pairings"
        );
        assert_eq!(
            report.non_clamp_axis_instances, 0,
            "a real texgen tile now has a non-clamp axis (PLAN.md R2.1/T7): this test's coverage assumes RE-230's \
             finding that every texgen tile clamps on both axes, and needs extending before this can pass"
        );
        // RE-231/T7a: a mask-narrowed clamp-without-mirror texgen axis used
        // to hold one texel short of real hardware exactly at the sweep's
        // dot=+1 extreme. Fixed in `n64_addressing::psp_lowering_axis` and
        // `texture::mirror_axis_len`/`mirror_fold`; back to a strict 0.
        assert_eq!(
            report.diverging_axis_instances, 0,
            "measured texgen addressing divergence count changed (PLAN.md R2.1/T7a, RE-231): investigate before \
             re-pinning a nonzero baseline"
        );
        assert_eq!(
            report.diverging_at_non_extreme_dot, 0,
            "a texgen addressing divergence now reaches beyond the sweep's dot=+1 extreme (PLAN.md R2.1/T7, \
             RE-231): this is a materially different, likely larger, gap than what RE-231 measured"
        );
    }

    /// `PLAN.md` R2.1/T7's own text asks for host/reference cases beyond
    /// what RE-230 measured real archive content actually uses (every real
    /// texgen tile clamps both axes, `texgen_addressing_census_against_real_archive_materials`
    /// above) -- repeat+mask and mirror+repeat with no clamp bit, which the
    /// real archive never exercises for texgen but which `address_axis`/
    /// `psp_lowering_axis` must still agree on, since a future asset or a
    /// currently-undiscovered display list could use them. Origin is held
    /// at 0 for every non-clamp case: `texgen_s10_5_addressed` only
    /// subtracts the tile origin on a clamp axis (RE-228), so a non-clamp
    /// axis with a nonzero origin is a real, currently-unsupported gap, not
    /// something this synthetic case should paper over.
    #[test]
    fn texgen_addressing_reference_cases_for_material_combinations_not_seen_in_the_real_archive() {
        // (name, mask, mirror, clamp_bit, origin, dim, scale)
        let cases: &[(&str, u8, bool, bool, u16, u16, u16)] = &[
            (
                "zero origin, clamp, full scale",
                5,
                false,
                true,
                0,
                32,
                0x07C0,
            ),
            (
                "nonzero origin, clamp, mask-narrowed scale (RE-214 file 117 shape)",
                4,
                false,
                true,
                6,
                16,
                0x0400,
            ),
            ("repeat + mask, no clamp", 5, false, false, 0, 32, 0x07C0),
            ("mirror + repeat, no clamp", 5, true, false, 0, 32, 0x07C0),
            ("mirror + clamp", 5, true, true, 0, 32, 0x07C0),
            (
                "padded PSP dim (42, non-power-of-two), clamp",
                6,
                false,
                true,
                0,
                42,
                0x0A40,
            ),
        ];

        for &(name, mask, mirror, clamp_bit, origin, dim, scale) in cases {
            let (origin_q2, far_edge_q2) = if clamp_bit {
                (0, (dim as i32 - 1) << 2)
            } else {
                (origin as i32, origin as i32 + ((dim as i32 - 1) << 2))
            };
            let model = ssb_rom::n64_addressing::TileAxis {
                shift: 0,
                origin_q2,
                far_edge_q2,
                mask,
                mirror,
                clamp_bit,
            };
            let period = 1u32 << mask;

            for linear in [false, true] {
                for n in -127i8..=127 {
                    let normal = [n, 0, 0];
                    let basis_s = [1.0, 0.0, 0.0];
                    let zero = [0.0, 0.0, 0.0];
                    let (u, _) = if linear {
                        ssb_rom::psp_texture::linear_texgen_uv(
                            normal, basis_s, zero, scale, scale, origin, origin, clamp_bit,
                            clamp_bit,
                        )
                    } else {
                        ssb_rom::psp_texture::regular_texgen_uv(
                            normal, basis_s, zero, scale, scale, origin, origin, clamp_bit,
                            clamp_bit,
                        )
                    };
                    let coord = u as i32;

                    let hw = ssb_rom::n64_addressing::address_axis(&model, coord);
                    let psp_rel = if clamp_bit {
                        coord
                    } else {
                        coord - origin as i32 * 8
                    };
                    let psp = ssb_rom::n64_addressing::psp_lowering_axis(
                        psp_rel, period, dim as u32, mirror, clamp_bit,
                    );
                    assert_eq!(
                        hw, psp,
                        "{name} (linear={linear}, n={n}): hardware {hw} vs PSP-lowered {psp}"
                    );
                }
            }
        }
    }

    /// RE-223/`R2.0`/P2: `G_SETTILE.palette` selects a 16-entry bank within
    /// a multi-bank loaded TLUT. Three distinct-coloured 16-entry banks,
    /// matching the real file 86 `ITCommonObject` shape RE-223 measured
    /// (three banks, real instances requesting bank 1) -- picking the wrong
    /// bank cannot pass this by accident.
    #[test]
    fn convert_texture_resolves_the_requested_palette_bank() {
        use ssb_rom::mesh::TextureRef;
        use ssb_rom::texture::{BitSize, Format};

        let mut data = vec![0u8; 5];
        data[4] = 0x00; // CI4 texel: high nibble -> palette index 0.
        for bank in 0..3u16 {
            let v: u16 = match bank {
                0 => 0x0001,
                1 => 0xFFFF,
                _ => 0x8000,
            };
            for _ in 0..16 {
                data.extend_from_slice(&v.to_be_bytes());
            }
        }
        let file = ssb_rom::archive::File {
            id: 0,
            data,
            extern_relocs: Vec::new(),
            intern_relocs: Vec::new(),
        };
        let texels = super::Texels {
            home: &file,
            all: &[],
        };

        let base = TextureRef {
            data_file: None,
            data_offset: 4,
            format: Format::Ci,
            size: BitSize::Bits4,
            width: 1,
            height: 1,
            source_width: 1,
            palette_file: None,
            palette_offset: Some(5),
            palette_entries: 48,
            palette: 1,
            mirror_s: false,
            mirror_t: false,
            clamp_s: false,
            clamp_t: false,
            framebuffer: false,
            origin_s: 0,
            origin_t: 0,
            mask_s: 0,
            mask_t: 0,
            drawn_width: 1,
            drawn_height: 1,
            tlut: ssb_rom::texture::TextureLut::Rgba16,
        };
        let expect_abgr = |v: u16| ssb_rom::psp_texture::pack_abgr(ssb_rom::texture::rgba5551(v));

        let packed = super::convert_texture(
            texels,
            &base,
            &super::filter_coverage::Coverage::default(),
            false,
            None,
            ssb_rom::filter_compensation::AlphaPolicy::Opaque,
            None,
            None,
        )
        .expect("CI4 texture converts");
        assert_eq!(
            packed.palette[0],
            expect_abgr(0xFFFF),
            "palette == 1 must read bank 1's colours, not bank 0's"
        );

        let bank0 = TextureRef { palette: 0, ..base };
        let packed0 = super::convert_texture(
            texels,
            &bank0,
            &super::filter_coverage::Coverage::default(),
            false,
            None,
            ssb_rom::filter_compensation::AlphaPolicy::Opaque,
            None,
            None,
        )
        .expect("CI4 texture converts");
        assert_eq!(
            packed0.palette[0],
            expect_abgr(0x0001),
            "palette == 0 (the overwhelming common case) must stay bank 0 -- a no-op"
        );

        // Guard: a bank beyond the loaded TLUT should not occur on real
        // content (RE-223's own measurement), but must not panic or index
        // out of bounds -- falls back to bank 0 rather than losing the
        // palette entirely.
        let out_of_range = TextureRef { palette: 5, ..base };
        let packed_guard = super::convert_texture(
            texels,
            &out_of_range,
            &super::filter_coverage::Coverage::default(),
            false,
            None,
            ssb_rom::filter_compensation::AlphaPolicy::Opaque,
            None,
            None,
        )
        .expect("an out-of-range bank must not panic");
        assert_eq!(packed_guard.palette[0], expect_abgr(0x0001));
    }

    /// One `convert_texture` call on a synthetic file, with the residual
    /// recorder on: the packed texture, whether a compensated result was
    /// kept, and the recorder's `(method, attempted, natural, final)`.
    fn convert_recorded(
        home: u32,
        data: Vec<u8>,
        t: &ssb_rom::mesh::TextureRef,
    ) -> (
        ssb_rom::psp_texture::PspTexture,
        bool,
        (
            &'static str,
            bool,
            ssb_rom::psp_texture::Psm,
            ssb_rom::psp_texture::Psm,
        ),
    ) {
        let file = ssb_rom::archive::File {
            id: home,
            data,
            extern_relocs: Vec::new(),
            intern_relocs: Vec::new(),
        };
        let texels = super::Texels {
            home: &file,
            all: &[],
        };
        super::residuals::enable();
        let mut compensated = false;
        let packed = super::convert_texture(
            texels,
            t,
            &super::filter_coverage::Coverage::default(),
            false,
            None,
            ssb_rom::filter_compensation::AlphaPolicy::Opaque,
            Some(&mut compensated),
            None,
        )
        .expect("texture converts");
        let recorded = super::residuals::take_pending().expect("conversion recorded");
        (packed, compensated, recorded)
    }

    /// A `width` x `height` tile-0 texture at `data_offset`, drawn with the
    /// given TLUT mode and no palette.
    fn synthetic_texture(
        format: ssb_rom::texture::Format,
        size: ssb_rom::texture::BitSize,
        width: u16,
        height: u16,
        data_offset: u32,
    ) -> ssb_rom::mesh::TextureRef {
        ssb_rom::mesh::TextureRef {
            data_file: None,
            data_offset,
            format,
            size,
            width,
            height,
            source_width: width,
            palette_file: None,
            palette_offset: None,
            palette_entries: 0,
            palette: 0,
            mirror_s: false,
            mirror_t: false,
            clamp_s: false,
            clamp_t: false,
            framebuffer: false,
            origin_s: 0,
            origin_t: 0,
            mask_s: 0,
            mask_t: 0,
            drawn_width: width,
            drawn_height: height,
            tlut: ssb_rom::texture::TextureLut::None,
        }
    }

    /// A 5-bit level per texel of a `width`-wide diagonal ridge: a
    /// smooth shape whose diagonal slope is where 3-point and bilinear
    /// filtering disagree, and where the compensator measurably helps.
    fn ridge(width: usize, height: usize) -> Vec<u8> {
        (0..width * height)
            .map(|i| {
                let (x, y) = ((i % width) as i32, (i / width) as i32);
                ((x - y).abs() * 5).min(31) as u8
            })
            .collect()
    }

    fn widen5(l: u8) -> u8 {
        (l << 3) | (l >> 2)
    }

    /// RE-313: an RGBA16 source enters the compensator and keeps its natural
    /// `Psm5551` format when the 5551-quantized result passes the gates.
    #[test]
    fn rgba16_compensation_is_kept_as_psm5551() {
        use ssb_rom::psp_texture::Psm;
        use ssb_rom::texture::{BitSize, Format};
        let data: Vec<u8> = ridge(8, 8)
            .into_iter()
            .map(|l| u16::from(l) << 11 | u16::from(l) << 6 | u16::from(l) << 1 | 1)
            .flat_map(u16::to_be_bytes)
            .collect();
        let data = [vec![0; 8], data].concat();
        let t = synthetic_texture(Format::Rgba, BitSize::Bits16, 8, 8, 8);
        let (packed, compensated, (method, attempted, natural, final_psm)) =
            convert_recorded(1, data, &t);
        assert!(attempted, "{method}");
        assert_eq!(natural, Psm::Psm5551);
        assert!(compensated, "{method}");
        assert_eq!(method, "direct-rgba5551");
        assert_eq!(final_psm, Psm::Psm5551);
        assert_eq!(packed.format, Psm::Psm5551);
    }

    /// RE-313: an RGBA16 texture the compensator cannot improve keeps its
    /// natural 5551 format, bit-exact at level 0, instead of the old forced
    /// RGBA8888 expansion.
    #[test]
    fn uncompensated_rgba16_is_not_promoted() {
        use ssb_rom::psp_texture::Psm;
        use ssb_rom::texture::{BitSize, Format};
        let data: Vec<u8> = [0u16; 4]
            .into_iter()
            .chain([0x7BC1u16; 64])
            .flat_map(u16::to_be_bytes)
            .collect();
        let t = synthetic_texture(Format::Rgba, BitSize::Bits16, 8, 8, 8);
        let (packed, compensated, (_, attempted, _, final_psm)) = convert_recorded(1, data, &t);
        assert!(attempted);
        assert!(!compensated);
        assert_eq!(final_psm, Psm::Psm5551);
        assert_eq!(packed.format, Psm::Psm5551);
        assert_eq!(
            u16::from_le_bytes([packed.data[0], packed.data[1]]),
            ssb_rom::psp_texture::pack_5551(ssb_rom::texture::rgba5551(0x7BC1))
        );
    }

    /// RE-313: RGBA32 and IA16/IA8 sources enter the same compensator and
    /// stay `Psm8888`, their natural PSP format.
    #[test]
    fn rgba32_and_ia_sources_enter_compensation() {
        use ssb_rom::psp_texture::Psm;
        use ssb_rom::texture::{BitSize, Format};
        let rgba32: Vec<u8> = ridge(8, 8)
            .into_iter()
            .flat_map(|l| [widen5(l), widen5(l), widen5(l), 255])
            .collect();
        let ia16: Vec<u8> = ridge(8, 8)
            .into_iter()
            .flat_map(|l| [widen5(l), 255])
            .collect();
        let ia8: Vec<u8> = ridge(8, 8)
            .into_iter()
            .map(|l| (l >> 1) << 4 | 0xF)
            .collect();
        for (format, size, data) in [
            (Format::Rgba, BitSize::Bits32, rgba32),
            (Format::Ia, BitSize::Bits16, ia16),
            (Format::Ia, BitSize::Bits8, ia8),
        ] {
            let t = synthetic_texture(format, size, 8, 8, 8);
            let data = [vec![0; 8], data].concat();
            let (packed, compensated, (method, attempted, natural, _)) =
                convert_recorded(1, data, &t);
            assert!(attempted, "{format:?}/{size:?}: {method}");
            assert!(compensated, "{format:?}/{size:?}: {method}");
            assert_eq!(natural, Psm::Psm8888);
            assert_eq!(packed.format, Psm::Psm8888);
        }
    }

    /// RE-313: the RE-283 bypass still avoids the paletted transport, but its
    /// texels now go through the compensator instead of around it.
    #[test]
    fn re283_bypass_textures_are_compensated() {
        use ssb_rom::psp_texture::Psm;
        use ssb_rom::texture::{BitSize, Format};
        const TEXELS: usize = 0xCF18;
        const PALETTE: usize = TEXELS + 128;
        let mut data = vec![0u8; PALETTE + 32];
        for (i, l) in ridge(16, 16).into_iter().enumerate() {
            let v = l >> 1;
            data[TEXELS + i / 2] |= if i % 2 == 0 { v << 4 } else { v };
        }
        for i in 0..16u16 {
            let l = i * 2 + 1;
            let at = PALETTE + i as usize * 2;
            data[at..at + 2].copy_from_slice(&(l << 11 | l << 6 | l << 1 | 1).to_be_bytes());
        }
        let t = ssb_rom::mesh::TextureRef {
            palette_offset: Some(PALETTE as u32),
            palette_entries: 16,
            tlut: ssb_rom::texture::TextureLut::Rgba16,
            ..synthetic_texture(Format::Ci, BitSize::Bits4, 16, 16, TEXELS as u32)
        };
        // File 324, offset 0xCF18: Link's boot, RE-283's first bypass.
        let (packed, compensated, (method, attempted, natural, final_psm)) =
            convert_recorded(324, data, &t);
        assert_eq!(
            natural,
            Psm::PsmT4,
            "the report still names the natural format"
        );
        assert_eq!(final_psm, Psm::Psm8888, "the bypass still avoids PsmT4");
        assert_eq!(packed.format, Psm::Psm8888);
        assert!(packed.palette.is_empty());
        assert!(attempted, "{method}");
        assert!(compensated, "{method}");
    }

    /// RE-313: an I4 texture drawn with the TLUT off packs the intensity ramp
    /// even when a TLUT is loaded; drawn with it on, it packs the TLUT.
    #[test]
    fn intensity_textures_pack_the_tlut_only_when_it_is_enabled() {
        use ssb_rom::texture::{BitSize, Format, TextureLut};
        // Texels at 8, TLUT at 16.
        let mut data = vec![0u8; 16 + 32];
        data[8..16].fill(0x0F);
        for i in 0..16u16 {
            let at = 16 + i as usize * 2;
            data[at..at + 2].copy_from_slice(&(0x0801 + i).to_be_bytes());
        }
        let off = synthetic_texture(Format::I, BitSize::Bits4, 4, 4, 8);
        let (packed_off, _, _) = convert_recorded(1, data.clone(), &off);
        assert_eq!(
            packed_off.palette,
            ssb_rom::psp_texture::intensity_palette(BitSize::Bits4)
        );

        let on = ssb_rom::mesh::TextureRef {
            palette_offset: Some(16),
            palette_entries: 16,
            tlut: TextureLut::Rgba16,
            ..off
        };
        let (packed_on, _, _) = convert_recorded(1, data, &on);
        assert_eq!(
            packed_on.palette[15],
            ssb_rom::psp_texture::pack_abgr(ssb_rom::texture::rgba5551(0x0801 + 15))
        );
    }

    /// RE-313: an index past a short TLUT reads the TMEM slot's measured stale
    /// value (`STALE_TLUT_ENTRIES`) in both the decoded reference and the
    /// packed palette. The level-0 index field then matches the reference.
    #[test]
    fn short_tlut_indices_use_the_measured_stale_entry() {
        use ssb_rom::texture::{BitSize, Format, TextureLut};
        // File 120's Break the Targets arrow: 3 entries loaded from 0x598.
        let mut data = vec![0u8; 0x5A8 + 8];
        for (i, e) in [0x294Au16, 0xA50D, 0x0001, 0x7BC1].into_iter().enumerate() {
            data[0x598 + i * 2..0x59A + i * 2].copy_from_slice(&e.to_be_bytes());
        }
        data[0x5A8..0x5A8 + 8].copy_from_slice(&[0x01, 0x23, 0x32, 0x10, 0x01, 0x23, 0x32, 0x10]);
        let t = ssb_rom::mesh::TextureRef {
            palette_offset: Some(0x598),
            palette_entries: 3,
            tlut: TextureLut::Rgba16,
            ..synthetic_texture(Format::Ci, BitSize::Bits4, 4, 4, 0x5A8)
        };
        let (packed, _, _) = convert_recorded(120, data.clone(), &t);
        assert_eq!(packed.palette.len(), 4);
        assert_eq!(
            packed.palette[3],
            ssb_rom::psp_texture::pack_abgr(ssb_rom::texture::rgba5551(0x1091)),
            "slot 3 is the measured stale TMEM entry, not the unloaded 0x7BC1"
        );
        // The same bytes under any other palette address have no measured
        // value: the slot stays transparent black, matching the decoder.
        let (unknown, _, _) = convert_recorded(7, data, &t);
        assert_eq!(unknown.palette[3], 0);
    }

    /// Converting the same input twice packs identical bytes.
    #[test]
    fn texture_conversion_is_deterministic() {
        use ssb_rom::texture::{BitSize, Format};
        let data: Vec<u8> = ridge(8, 8)
            .into_iter()
            .map(|l| u16::from(l) << 11 | u16::from(l) << 6 | u16::from(l) << 1 | 1)
            .flat_map(u16::to_be_bytes)
            .collect();
        let data = [vec![0; 8], data].concat();
        let t = synthetic_texture(Format::Rgba, BitSize::Bits16, 8, 8, 8);
        let (a, _, _) = convert_recorded(1, data.clone(), &t);
        let (b, _, _) = convert_recorded(1, data, &t);
        assert_eq!(a.data, b.data);
        assert_eq!(a.palette, b.palette);
        assert_eq!(a.format, b.format);
    }

    #[test]
    fn padded_source_rows_do_not_apply_the_tile_origin_twice() {
        use ssb_rom::mesh::TextureRef;
        use ssb_rom::texture::{BitSize, Format};

        // Two 4-texel RGBA16 source rows, rendered through a 2-wide tile.
        // The nonzero origin is already represented by the packed UVs; it
        // must not also skip the first two pixels in this source buffer.
        let source = [
            0xF801u16, 0x07C1, 0x003F, 0xFFFF, // red, green, blue, white
            0xFFFF, 0x003F, 0x07C1, 0xF801, // white, blue, green, red
        ]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
        let texture = TextureRef {
            data_file: None,
            data_offset: 0,
            format: Format::Rgba,
            size: BitSize::Bits16,
            width: 2,
            height: 2,
            source_width: 4,
            palette_file: None,
            palette_offset: None,
            palette_entries: 0,
            palette: 0,
            mirror_s: false,
            mirror_t: false,
            clamp_s: true,
            clamp_t: true,
            framebuffer: false,
            origin_s: 2 << 2,
            origin_t: 0,
            mask_s: 0,
            mask_t: 0,
            drawn_width: 2,
            drawn_height: 2,
            tlut: ssb_rom::texture::TextureLut::None,
        };

        let image = super::decode_texture(&source, &texture, &[]).expect("texture decodes");
        assert_eq!(
            image.pixels,
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, // source row 0, columns 0..2
                255, 255, 255, 255, 0, 0, 255, 255, // source row 1, columns 0..2
            ],
            "the visible tile starts at the source buffer's top-left"
        );
    }
}
