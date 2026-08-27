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
//! romtool stages   <rom>          recover MPGroundData headers and collision
//! romtool collide  <pack>         run the collision query on every stage
//! romtool simulate <pack>         drop a real fighter on every stage's spawns
//! romtool fighters <rom>          extract every character's FTAttributes
//! romtool anims    <rom>          read every fighter's animation lengths
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ssb_rom::archive::Archive;
use ssb_rom::rom;

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
        ["stages", rom_path, rest @ ..] => stages(rom_path.as_ref(), rest),
        ["pack", rom_path, rest @ ..] => pack(rom_path.as_ref(), rest),
        ["collide", pack_path, rest @ ..] => collide(pack_path.as_ref(), rest),
        ["simulate", pack_path, rest @ ..] => simulate(pack_path.as_ref(), rest),
        ["fighters", rom_path, rest @ ..] => fighters(rom_path.as_ref(), rest),
        ["anims", rom_path, rest @ ..] => anims(rom_path.as_ref(), rest),
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
    romtool stages   <rom.z64> [--file <id>] [--lines]
    romtool pack     <rom.z64> [--out <file>] [--file <id>] [--no-swizzle]
    romtool collide  <pack.pak> [--stage <n>]
    romtool simulate <pack.pak> [--stage <n>] [--verbose]
    romtool fighters <rom.z64> [--verify] [--refs <relocData dir>]
    romtool anims    <rom.z64> [--verify]
    romtool figatree <rom.z64> [--fighter <name>] [--slot <name>] [--frames <n>]
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
        0x0000_0002 => "G_SHADE",
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
            let items: Vec<ssb_rom::mesh::SequenceItem> = plan
                .iter()
                .zip(&decoded)
                .map(|(p, cmds)| ssb_rom::mesh::SequenceItem {
                    cmds,
                    world: p.world,
                    mobjs: &materials[p.node],
                })
                .collect();

            for (p, converted) in plan.iter().zip(ssb_rom::mesh::convert_sequence(
                &items,
                ssb_rom::mesh::Source::of(file),
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
        let own = |dl| PlannedList {
            node: i,
            dl,
            space: Some(i),
            world: worlds[i],
        };
        match resolver.resolve(node_dl) {
            NodeDl::Direct(dl) => out.push(own(dl)),
            NodeDl::Links(links) => out.extend(links.iter().filter_map(|l| l.dl).map(own)),
            NodeDl::Pair { pre, post } => {
                if let Some(dl) = pre {
                    out.push(PlannedList {
                        node: i,
                        dl,
                        space: node.parent,
                        world: node.parent.map_or(Mat4::IDENTITY, |p| worlds[p]),
                    });
                }
                // The node's matrix is pushed between the two, so even when
                // `post` is NULL the node still occupies a step in the walk.
                out.push(own(post.unwrap_or(NO_LIST)));
            }
        }
    }
    out
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
fn pack_mesh(
    writer: &mut ssb_rom::pack::PackWriter,
    tex_index: &mut BTreeMap<(u32, u32), u32>,
    src: Texels<'_>,
    id: u32,
    offset: u32,
    m: &ssb_rom::mesh::Mesh,
    swizzle: bool,
) -> u32 {
    let mut per_prim: Vec<Option<u32>> = Vec::with_capacity(m.primitives.len());
    for prim in &m.primitives {
        per_prim.push(match prim.material.texture {
            None => None,
            Some(t) => {
                let key = (t.data_file.map_or(id, u32::from), t.data_offset);
                if let Some(&i) = tex_index.get(&key) {
                    Some(i)
                } else {
                    convert_texture(src, &t, swizzle).map(|tex| {
                        let i = writer.add_texture(&tex);
                        tex_index.insert(key, i);
                        i
                    })
                }
            }
        });
    }
    writer.add_mesh(m, id, offset, |i| per_prim[i])
}

/// Builds the runtime asset pack: converted geometry and textures in the
/// layout the PSP consumes directly.
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
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let mut writer = fmt::PackWriter::new();
    // The same texture is bound by many primitives; upload each once.
    let mut tex_index: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut meshes = 0usize;
    let mut triangles = 0usize;
    let mut objects = 0usize;
    let mut placed_meshes = 0usize;
    let mut node_dls = 0usize;
    let mut extra_leaves = 0usize;
    // A stage names its render layers by the `DObjDesc` address they start at,
    // and `add_object` is given that same address -- so the layer lookup is an
    // exact match, never a search.
    let mut object_index: BTreeMap<(u32, u32), u32> = BTreeMap::new();

    let loaded = load_all(&archive);

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
            let items: Vec<mesh::SequenceItem> = plan
                .iter()
                .zip(&decoded)
                .map(|(p, cmds)| mesh::SequenceItem {
                    cmds,
                    world: p.world,
                    mobjs: &materials[p.node],
                })
                .collect();

            for (p, converted) in plan
                .iter()
                .zip(mesh::convert_sequence(&items, mesh::Source::of(file)))
            {
                let Ok(m) = converted else { continue };
                if m.triangle_count() == 0 {
                    continue;
                }
                let index = pack_mesh(
                    &mut writer,
                    &mut tex_index,
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
            let Ok(m) = mesh::convert(&dl.commands, mesh::Source::of(file)) else {
                continue;
            };
            if m.triangle_count() == 0 {
                continue;
            }
            pack_mesh(
                &mut writer,
                &mut tex_index,
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

        for (gi, graph) in graphs.iter().enumerate() {
            node_dls += graph.display_lists().count();
            placed_meshes += node_mesh[gi].iter().filter(|m| m.is_some()).count();
            let object = writer.add_object(graph, id, |n| node_mesh[gi][n], &node_extra[gi]);
            object_index.insert((id, graph.offset), object);
            objects += 1;
        }
    }

    // Stages last: a layer can only be resolved once every object exists.
    let mut stage_layers = 0usize;
    let mut resolved_layers = 0usize;
    let mut with_collision = 0usize;
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
        writer.add_stage(ground, map.as_ref(), |file, offset| {
            object_index.get(&(file, offset)).copied()
        });
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
    Ok(())
}

/// Decodes and packs one texture referenced by a primitive.
///
/// The texels and the palette are looked up independently, because they need
/// not be in the same file: a fighter's palette comes from its own file while
/// a stage's texels come from a shared one.
fn convert_texture(
    src: Texels<'_>,
    t: &ssb_rom::mesh::TextureRef,
    swizzle: bool,
) -> Option<ssb_rom::psp_texture::PspTexture> {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture;

    let file = src.bytes(t.data_file)?;

    if (t.data_offset >> 24) != 0 || (t.data_offset == 0 && t.data_file.is_none()) {
        return None; // segmented, or a pointer nothing resolved
    }
    let psm = psp::choose_psm(t.format, t.size);
    let need = texture::data_len(t.width as u32, t.height as u32, t.size);
    let texels = file.get(t.data_offset as usize..t.data_offset as usize + need)?;

    let tlut: Vec<u16> = match t.palette_offset {
        Some(off) => {
            let n = t.palette_entries.max(1) as usize;
            src.bytes(t.palette_file)
                .and_then(|f| f.get(off as usize..off as usize + n * 2))
                .map(texture::parse_tlut)
                .unwrap_or_default()
        }
        None => Vec::new(),
    };

    if psm.is_paletted() && !tlut.is_empty() {
        psp::pack_paletted(
            texels,
            t.width as u32,
            t.height as u32,
            t.size,
            &tlut,
            swizzle,
        )
        .ok()
    } else {
        texture::decode(
            texels,
            t.width as u32,
            t.height as u32,
            t.format,
            t.size,
            (!tlut.is_empty()).then_some(tlut.as_slice()),
        )
        .ok()
        .map(|img| psp::pack_rgba(&img, psp::Psm::Psm8888, swizzle))
    }
}

/// The whole archive, read once.
///
/// Material tables have to be resolved before any model can be converted,
/// because the `FTCommonPart` record that names a model's table lives in the
/// fighter's `*Main` file, not the `*Model` file it describes. Decompressing
/// everything up front costs about 17 MB of RAM and saves a second pass.
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
    let graphs: BTreeMap<u32, Vec<scene::SceneGraph>> = files
        .iter()
        .flatten()
        .map(|f| (f.id, scene::find_scene_graphs(f)))
        .collect();
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

    Loaded {
        files,
        graphs,
        tables,
        stages,
    }
}

impl Loaded {
    /// The materials for each node of a graph, or empty vectors when no record
    /// names its table.
    fn materials(
        &self,
        file: &ssb_rom::archive::File,
        graph: &ssb_rom::scene::SceneGraph,
    ) -> Vec<ssb_rom::mobj::NodeMaterials> {
        self.tables
            .table_for(file.id, graph.offset)
            .and_then(|at| ssb_rom::mobj::read_table(file, at, graph.nodes.len()))
            .map(|t| t.nodes)
            .unwrap_or_else(|| vec![Vec::new(); graph.nodes.len()])
    }
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
        let items: Vec<mesh::SequenceItem> = plan
            .iter()
            .zip(&decoded)
            .map(|(p, cmds)| mesh::SequenceItem {
                cmds,
                world: p.world,
                mobjs: &materials[p.node],
            })
            .collect();
        out.extend(
            mesh::convert_sequence(&items, mesh::Source::of(file))
                .into_iter()
                .flatten(),
        );
    }

    for dl in ssb_rom::scan::find_root_display_lists(file) {
        if !claimed.contains(&dl.offset) {
            out.extend(mesh::convert(&dl.commands, mesh::Source::of(file)));
        }
    }
    out
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
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            "--lines" => verbose = true,
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    let loaded = load_all(&archive);

    let mut layers = 0usize;
    let mut with_table = 0usize;
    let mut collision_ok = 0usize;
    let mut collision_bad = 0usize;
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
            println!(
                "  layer {}  graph file {} @ 0x{:X} ({nodes} nodes)  {table}",
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
    println!("collision maps decoded: {collision_ok}, failed: {collision_bad}");
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
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            "--expect" => expect = it.next().map(PathBuf::from),
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
    let (mut agree, mut disagree, mut unfollowable) = (0usize, 0usize, 0usize);
    let (mut materials, mut palettes) = (0usize, 0usize);
    let (mut in_decomp, mut not_in_decomp) = (0usize, 0usize);

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
                    palettes += usize::from(m.palette.is_some());
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
                if !seen.insert((id, t.data_offset)) {
                    continue;
                }
                if (t.data_offset >> 24) != 0 || t.data_offset == 0 {
                    continue;
                }

                let need = texture::data_len(t.width as u32, t.height as u32, t.size);
                let Some(src) = file
                    .data
                    .get(t.data_offset as usize..t.data_offset as usize + need)
                else {
                    continue;
                };
                let tlut: Vec<u16> = match t.palette_offset {
                    Some(off) => {
                        let n = t.palette_entries.max(1) as usize;
                        file.data
                            .get(off as usize..off as usize + n * 2)
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
                let name = format!(
                    "f{id}_o{:X}_{:?}{}_{}x{}.ppm",
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
    let mut seen: std::collections::BTreeSet<(u32, u32)> = std::collections::BTreeSet::new();
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

    // Same conversion the packer runs, so these counts describe the pack. On
    // the standalone path a fighter's textures come out palette-less, and this
    // reported 142 failures that the shipped pack does not have.
    for file in loaded.files.iter().flatten() {
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
                if !seen.insert((home, t.data_offset)) {
                    continue;
                }
                let texels = match t.data_file {
                    None => &file.data[..],
                    Some(id) => match loaded.files.get(id as usize).and_then(Option::as_ref) {
                        Some(f) => &f.data[..],
                        None => {
                            *why.entry("cross-file, target file did not load".into())
                                .or_default() += 1;
                            decode_failed += 1;
                            continue;
                        }
                    },
                };

                let psm = psp::choose_psm(t.format, t.size);
                let need = texture::data_len(t.width as u32, t.height as u32, t.size);
                let at = t.data_offset as usize;

                // Diagnose *why* a texture cannot be read, rather than lumping
                // every failure together.
                per_file.entry(file.id).or_default().0 += 1;
                let segment = (t.data_offset >> 24) as u8;
                if segment != 0 {
                    *why.entry(format!("segmented addr (seg 0x{segment:02X})"))
                        .or_default() += 1;
                    decode_failed += 1;
                    continue;
                }
                if t.data_offset == 0 && t.data_file.is_none() {
                    *why.entry("null pointer, nothing resolved it".into())
                        .or_default() += 1;
                    decode_failed += 1;
                    continue;
                }
                let Some(src) = texels.get(at..at + need) else {
                    *why.entry("offset past end of file".into()).or_default() += 1;
                    decode_failed += 1;
                    continue;
                };
                if psm.is_paletted() && t.palette_offset.is_none() {
                    *why.entry("paletted but no TLUT recorded".into())
                        .or_default() += 1;
                }

                // Palette, if this is a CLUT format.
                let tlut: Vec<u16> = match t.palette_offset {
                    Some(off) => {
                        let entries = t.palette_entries.max(1) as usize;
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

                let tex = if psm.is_paletted() && !tlut.is_empty() {
                    match psp::pack_paletted(
                        src,
                        t.width as u32,
                        t.height as u32,
                        t.size,
                        &tlut,
                        true,
                    ) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            *why.entry(format!("pack_paletted: {e:?}")).or_default() += 1;
                            None
                        }
                    }
                } else {
                    match texture::decode(
                        src,
                        t.width as u32,
                        t.height as u32,
                        t.format,
                        t.size,
                        (!tlut.is_empty()).then_some(tlut.as_slice()),
                    ) {
                        Ok(img) => Some(psp::pack_rgba(&img, psp::Psm::Psm8888, true)),
                        Err(e) => {
                            *why.entry(format!("decode: {e:?}")).or_default() += 1;
                            None
                        }
                    }
                };

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
    for (reason, n) in &why {
        println!("    {reason:<48} {n:>4}");
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
                looping.push((entry.name, anim::SLOT_NAMES[slot]));
                print!(" {:>9}", "loops");
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
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--fighter" => want_fighter = it.next().map(|s| s.to_string()),
            "--slot" => want_slot = it.next().map(|s| s.to_string()),
            "--frames" => frames = it.next().ok_or("--frames needs a count")?.parse()?,
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
            let file = archive.load(id as u32)?;
            let table = joint_table(&file.data).ok_or("no joint table")?;
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
